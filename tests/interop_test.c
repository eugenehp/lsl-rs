/**
 * Cross-process interop test — verifies the Rust liblsl.dylib/so works
 * when loaded from C, matching the official liblsl C API.
 *
 * Build: cc -o interop_test tests/interop_test.c -Ltarget/debug -llsl -Wl,-rpath,target/debug
 * Run:   ./interop_test
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* Declare the liblsl C API we're testing */
typedef void* lsl_streaminfo;
typedef void* lsl_outlet;
typedef void* lsl_inlet;
typedef void* lsl_xml_ptr;

extern int    lsl_protocol_version(void);
extern int    lsl_library_version(void);
extern const char* lsl_library_info(void);
extern double lsl_local_clock(void);

extern lsl_streaminfo lsl_create_streaminfo(const char* name, const char* type, int nch, double srate, int fmt, const char* src_id);
extern void   lsl_destroy_streaminfo(lsl_streaminfo info);
extern lsl_streaminfo lsl_copy_streaminfo(lsl_streaminfo info);
extern const char* lsl_get_name(lsl_streaminfo info);
extern const char* lsl_get_type(lsl_streaminfo info);
extern int    lsl_get_channel_count(lsl_streaminfo info);
extern double lsl_get_nominal_srate(lsl_streaminfo info);
extern int    lsl_get_channel_format(lsl_streaminfo info);
extern const char* lsl_get_uid(lsl_streaminfo info);
extern const char* lsl_get_xml(lsl_streaminfo info);
extern int    lsl_stream_info_matches_query(lsl_streaminfo info, const char* query);
extern lsl_xml_ptr lsl_get_desc(lsl_streaminfo info);

extern lsl_outlet lsl_create_outlet(lsl_streaminfo info, int chunk_size, int max_buffered);
extern void   lsl_destroy_outlet(lsl_outlet out);
extern void   lsl_push_sample_ftp(lsl_outlet out, const float* data, double ts, int pushthrough);
extern int    lsl_have_consumers(lsl_outlet out);

extern int    lsl_resolve_all(lsl_streaminfo* buffer, unsigned int buffer_elements, double wait_time);

extern lsl_inlet lsl_create_inlet(lsl_streaminfo info, int max_buflen, int max_chunklen, int recover);
extern void   lsl_destroy_inlet(lsl_inlet in_);
extern void   lsl_open_stream(lsl_inlet in_, double timeout, int* ec);
extern double lsl_pull_sample_f(lsl_inlet in_, float* buffer, int buffer_elements, double timeout, int* ec);
extern void   lsl_close_stream(lsl_inlet in_);
extern double lsl_time_correction(lsl_inlet in_, double timeout, int* ec);
extern void   lsl_set_postprocessing(lsl_inlet in_, unsigned int flags);

extern lsl_xml_ptr lsl_append_child_value(lsl_xml_ptr e, const char* name, const char* value);
extern const char* lsl_child_value_n(lsl_xml_ptr e, const char* name);
extern int    lsl_empty(lsl_xml_ptr e);

#define CHECK(cond, msg) do { if (!(cond)) { fprintf(stderr, "FAIL: %s\n", msg); failures++; } else { printf("  ✓ %s\n", msg); } } while(0)

int main() {
    int failures = 0;
    printf("=== liblsl C interop test ===\n\n");

    /* 1. Version & clock */
    printf("1. Basic functions\n");
    int pv = lsl_protocol_version();
    CHECK(pv == 110, "protocol_version == 110");
    int lv = lsl_library_version();
    CHECK(lv > 0, "library_version > 0");
    const char* li = lsl_library_info();
    CHECK(li != NULL && strlen(li) > 0, "library_info not empty");
    double t = lsl_local_clock();
    CHECK(t > 0, "local_clock > 0");

    /* 2. StreamInfo */
    printf("\n2. StreamInfo\n");
    lsl_streaminfo info = lsl_create_streaminfo("CTest", "EEG", 4, 250.0, 1, "c_interop");
    CHECK(info != NULL, "create_streaminfo");
    CHECK(strcmp(lsl_get_name(info), "CTest") == 0, "name == CTest");
    CHECK(strcmp(lsl_get_type(info), "EEG") == 0, "type == EEG");
    CHECK(lsl_get_channel_count(info) == 4, "channel_count == 4");
    CHECK(lsl_get_nominal_srate(info) == 250.0, "srate == 250");
    CHECK(lsl_get_channel_format(info) == 1, "format == float32 (1)");
    CHECK(strlen(lsl_get_uid(info)) > 5, "uid not empty");

    /* 2b. matches_query */
    CHECK(lsl_stream_info_matches_query(info, "name='CTest'") == 1, "matches name='CTest'");
    CHECK(lsl_stream_info_matches_query(info, "name='Other'") == 0, "!matches name='Other'");
    CHECK(lsl_stream_info_matches_query(info, "") == 1, "matches empty query");

    /* 2c. Deep copy */
    lsl_streaminfo copy = lsl_copy_streaminfo(info);
    CHECK(copy != NULL, "copy_streaminfo");
    CHECK(strcmp(lsl_get_name(copy), "CTest") == 0, "copy name preserved");
    lsl_destroy_streaminfo(copy);

    /* 2d. XML */
    const char* xml = lsl_get_xml(info);
    CHECK(xml != NULL && strstr(xml, "<name>CTest</name>") != NULL, "get_xml contains name");

    /* 2e. XML DOM */
    lsl_xml_ptr desc = lsl_get_desc(info);
    CHECK(desc != NULL, "get_desc");
    lsl_xml_ptr ch_node = lsl_append_child_value(desc, "manufacturer", "RustLSL");
    CHECK(ch_node != NULL, "append_child_value");
    const char* mfr = lsl_child_value_n(desc, "manufacturer");
    CHECK(strcmp(mfr, "RustLSL") == 0, "child_value_n == RustLSL");

    /* 3. Outlet + Inlet loopback */
    printf("\n3. Outlet→Inlet loopback\n");
    lsl_outlet out = lsl_create_outlet(info, 0, 360);
    CHECK(out != NULL, "create_outlet");

    /* Push some data */
    float sample[4] = {1.0f, 2.0f, 3.0f, 4.0f};
    for (int i = 0; i < 50; i++) {
        lsl_push_sample_ftp(out, sample, 0.0, 1);
    }
    usleep(500000); /* 500ms for discovery */

    /* Resolve (shorter timeout) */
    lsl_streaminfo results[8];
    int n = lsl_resolve_all(results, 8, 1.5);
    CHECK(n > 0, "resolve_all found streams");

    lsl_streaminfo found = NULL;
    for (int i = 0; i < n; i++) {
        if (strcmp(lsl_get_name(results[i]), "CTest") == 0) {
            found = results[i];
            break;
        }
    }
    CHECK(found != NULL, "found CTest stream");

    if (found) {
        /* Create inlet */
        lsl_inlet in_ = lsl_create_inlet(found, 360, 0, 1);
        CHECK(in_ != NULL, "create_inlet");

        int ec = 0;
        lsl_open_stream(in_, 5.0, &ec);
        CHECK(ec == 0, "open_stream ok");

        /* Push more samples for the inlet */
        for (int i = 0; i < 100; i++) {
            sample[0] = (float)i;
            lsl_push_sample_ftp(out, sample, 0.0, 1);
        }
        usleep(200000);

        /* Pull */
        float buf[4] = {0};
        double ts = lsl_pull_sample_f(in_, buf, 4, 2.0, &ec);
        CHECK(ts > 0.0, "pull_sample_f got data");
        CHECK(ec == 0, "pull_sample_f no error");

        /* Post-processing */
        lsl_set_postprocessing(in_, 1|2|4); /* clocksync|dejitter|monotonize */

        /* Time correction (short timeout) */
        double tc = lsl_time_correction(in_, 1.0, &ec);
        /* tc can be 0 for localhost, just check it doesn't crash */
        printf("  time_correction = %.6f\n", tc);
        CHECK(ec == 0, "time_correction no error");

        lsl_close_stream(in_);
        lsl_destroy_inlet(in_);
    }

    /* Cleanup */
    for (int i = 0; i < n; i++) lsl_destroy_streaminfo(results[i]);
    lsl_destroy_outlet(out);
    lsl_destroy_streaminfo(info);

    printf("\n=== Results: %d failure(s) ===\n", failures);
    return failures;
}
