//! WASM client — connects to `lsl-bridge` from the browser.
//!
//! Usage in JavaScript (after wasm-pack build):
//!
//! ```js
//! import init, { LslClient } from "./pkg/lsl_wasm.js";
//!
//! await init();
//! const client = new LslClient("ws://localhost:8765");
//!
//! // List available streams
//! const streams = await client.list_streams();
//! console.log(streams);
//!
//! // Subscribe with a callback
//! client.subscribe(streams[0].uid, (streamId, timestamps, data) => {
//!     console.log(`Got ${timestamps.length} samples from ${streamId}`);
//! });
//!
//! // Later: unsubscribe
//! client.unsubscribe(streams[0].uid);
//! client.close();
//! ```

#[cfg(feature = "wasm")]
mod inner {
    use crate::protocol::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{MessageEvent, WebSocket};

    /// LSL WebSocket client for the browser.
    #[wasm_bindgen]
    pub struct LslClient {
        ws: WebSocket,
        /// JS callback: fn(stream_id: string, timestamps: Float64Array, data: Float64Array, nch: number)
        on_data: Option<js_sys::Function>,
        /// Pending list response
        _list_resolve: Option<js_sys::Function>,
    }

    #[wasm_bindgen]
    impl LslClient {
        /// Connect to an lsl-bridge server.
        #[wasm_bindgen(constructor)]
        pub fn new(url: &str) -> Result<LslClient, JsValue> {
            let ws = WebSocket::new(url)?;
            ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

            Ok(LslClient {
                ws,
                on_data: None,
                _list_resolve: None,
            })
        }

        /// Set up the internal message handler. Must be called after construction.
        #[wasm_bindgen]
        pub fn init(&mut self) {
            let on_data = self.on_data.clone();

            let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    let s: String = text.into();
                    if let Ok(msg) = serde_json::from_str::<ServerMsg>(&s) {
                        match msg {
                            ServerMsg::Data {
                                stream_id,
                                timestamps,
                                data,
                            } => {
                                if let Some(ref cb) = on_data {
                                    let nch = if data.is_empty() { 0 } else { data[0].len() };
                                    // Flatten data to a single Float64Array
                                    let flat: Vec<f64> =
                                        data.into_iter().flat_map(|row| row).collect();
                                    let ts_arr = js_sys::Float64Array::from(timestamps.as_slice());
                                    let data_arr = js_sys::Float64Array::from(flat.as_slice());
                                    let _ = cb.call4(
                                        &JsValue::NULL,
                                        &JsValue::from_str(&stream_id),
                                        &ts_arr,
                                        &data_arr,
                                        &JsValue::from_f64(nch as f64),
                                    );
                                }
                            }
                            ServerMsg::Streams { streams } => {
                                // Convert to JSON and log
                                if let Ok(json) = serde_json::to_string(&streams) {
                                    web_sys::console::log_1(
                                        &format!("LSL streams: {}", json).into(),
                                    );
                                }
                            }
                            ServerMsg::Error { message } => {
                                web_sys::console::error_1(
                                    &format!("LSL bridge error: {}", message).into(),
                                );
                            }
                        }
                    }
                }
            });

            self.ws
                .set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget(); // prevent GC

            let onerror =
                Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |e: web_sys::ErrorEvent| {
                    web_sys::console::error_1(&format!("WS error: {:?}", e.message()).into());
                });
            self.ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();
        }

        /// Request the list of streams. The result is logged to the console.
        #[wasm_bindgen]
        pub fn list_streams(&self) -> Result<(), JsValue> {
            let msg = serde_json::to_string(&ClientMsg::List)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.ws.send_with_str(&msg)
        }

        /// Subscribe to a stream by UID. Set the data callback first with `set_on_data`.
        #[wasm_bindgen]
        pub fn subscribe(&self, stream_id: &str) -> Result<(), JsValue> {
            let msg = serde_json::to_string(&ClientMsg::Subscribe {
                stream_id: stream_id.to_string(),
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.ws.send_with_str(&msg)
        }

        /// Unsubscribe from a stream.
        #[wasm_bindgen]
        pub fn unsubscribe(&self, stream_id: &str) -> Result<(), JsValue> {
            let msg = serde_json::to_string(&ClientMsg::Unsubscribe {
                stream_id: stream_id.to_string(),
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.ws.send_with_str(&msg)
        }

        /// Set the data callback: `fn(streamId: string, timestamps: Float64Array, data: Float64Array, nch: number)`.
        #[wasm_bindgen]
        pub fn set_on_data(&mut self, callback: js_sys::Function) {
            self.on_data = Some(callback);
        }

        /// Close the WebSocket connection.
        #[wasm_bindgen]
        pub fn close(&self) -> Result<(), JsValue> {
            self.ws.close()
        }
    }
}
