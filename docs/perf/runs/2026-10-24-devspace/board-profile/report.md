# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| board-2000 | None ms | None MiB | None | None | None MiB | None | None |

## Board opening: navigate → first card drawn and laid out

| row | tasks | cold mount, median of 3 launches | each launch's cold | reopen, median | reopen, max |
|---|---:|---:|---|---:|---:|
| board-2000 | 2000 | 570.5 ms | 565, 570, 576 | 11.1 ms | 11.8 ms |

## board-2000: CPU by shared object

- libgtk-4.so.1.2400.1 **35.35%**
- libgobject-2.0.so.0.9000.1 **18.77%**
- libc.so.6 **13.71%**
- libglib-2.0.so.0.9000.1 **9.19%**
- relay-native **5.51%**
- libpango-1.0.so.0.5800.2 **5.13%**
- libharfbuzz.so.0.61460.0 **4.06%**
- libcairo.so.2.11806.6 **2.33%**
- libpng16.so.16.59.0 **1.27%**
- libgraphene-1.0.so.0.1000.8 **0.8%**
- libz.so.1.3.1.zlib-ng **0.64%**
- libfontconfig.so.1.17.0 **0.59%**

### board-2000: hottest symbols

- `libgobject-2.0.so.0.9000.1 [.] g_object_ref                                                                                                 ` 4.8%
- `libgobject-2.0.so.0.9000.1 [.] g_object_unref                                                                                               ` 4.45%
- `libc.so.6 [.] _int_free_chunk                                                                                                               ` 3.26%
- `libc.so.6 [.] _int_malloc                                                                                                                   ` 2.69%
- `libc.so.6 [.] __libc_malloc2                                                                                                                ` 2.15%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003974a0                                                                                                 ` 2.13%
- `libgobject-2.0.so.0.9000.1 [.] g_type_check_instance_is_a                                                                                   ` 2.0%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003974a4                                                                                                 ` 1.49%
- `libglib-2.0.so.0.9000.1 [.] g_mutex_unlock                                                                                                  ` 1.43%
- `libgobject-2.0.so.0.9000.1 [.] 0x0000000000042181                                                                                           ` 1.41%
- `libgobject-2.0.so.0.9000.1 [.] g_type_check_instance_is_fundamentally_a                                                                     ` 1.33%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000109b7a                                                                                                 ` 1.28%
- `libpng16.so.16.59.0 [.] png_write_row                                                                                                       ` 1.25%
- `libglib-2.0.so.0.9000.1 [.] g_atomic_ref_count_inc                                                                                          ` 1.17%
- `libc.so.6 [.] __libc_calloc2                                                                                                                ` 1.1%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039dd65                                                                                                 ` 1.09%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039dd6b                                                                                                 ` 1.06%
- `libglib-2.0.so.0.9000.1 [.] g_atomic_ref_count_dec                                                                                          ` 1.0%
- `libglib-2.0.so.0.9000.1 [.] g_pointer_bit_unlock                                                                                            ` 0.99%
- `relay-native [.] <indexmap::inner::Core<alloc::string::String, serde_json::value::Value>>::get_index_of::<str>                              ` 0.96%
