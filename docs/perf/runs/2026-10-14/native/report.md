# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf on.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents | 610.0 ms | 220.5 MiB | 0.0 | 1.7 | 220.5 MiB | 17 | 0 |
| board | 330.0 ms | 177.8 MiB | 0.37 | 8.0 | 181.0 MiB | 13 | 0 |
| code | 350.0 ms | 180.8 MiB | 0.12 | 6.0 | 181.1 MiB | 13 | 0 |
| notes | 330.0 ms | 178.0 MiB | 0.25 | 1.2 | 178.0 MiB | 13 | 0 |
| modules | 320.0 ms | 176.5 MiB | 0.12 | 2.2 | 176.9 MiB | 13 | 0 |
| settings | 320.0 ms | 176.4 MiB | 0.62 | 29.6 | 194.0 MiB | 28 | 0 |
| skills | 370.0 ms | 183.9 MiB | 0.99 | 64.4 | 184.6 MiB | 26 | 0 |
| dashboard | 350.0 ms | 178.7 MiB | 0.87 | 60.1 | 178.7 MiB | 13 | 0 |
| notifications | 350.0 ms | 177.0 MiB | 1.24 | 67.8 | 177.5 MiB | 13 | 0 |
| devices | 340.0 ms | 177.6 MiB | 1.12 | 72.2 | 179.8 MiB | 14 | 0 |
| launch-preview | 340.0 ms | 178.6 MiB | 0.37 | 4.9 | 181.9 MiB | 14 | 0 |
| palette | 360.0 ms | 178.7 MiB | 1.62 | 125.7 | 179.5 MiB | 13 | 0 |
| layouts | 340.0 ms | 178.8 MiB | 0.37 | 4.2 | 190.2 MiB | 13 | 0 |
| agents-streaming | 360.0 ms | 178.8 MiB | 1.49 | 103.6 | 180.2 MiB | 13 | 0 |
| agents-burst | 360.0 ms | 190.8 MiB | 0.37 | 61.9 | 192.0 MiB | 13 | 0 |

## agents: CPU by shared object

- relay-native **91.41%**
- [unknown] **7.1%**
- libc.so.6 **1.49%**

### agents: hottest symbols

- `relay-native [.] <glib::auto::main_context::MainContext>::spawn_local_with_priority::<(), <relay_native::app::Ui>::save_layout::{closure#0}>` 89.96%
- `[unknown] [k] 0xffffffff8edb9759                                                                                                           -` 2.66%
- `libc.so.6 [.] 0x00000000000b6132                                                                                                           -` 1.49%
- `relay-native [.] <tokio::runtime::io::driver::Driver>::turn                                                                                 ` 1.45%
- `[unknown] [k] 0xffffffff8ed7b90f                                                                                                           -` 1.36%
- `[unknown] [k] 0xffffffff8edb995a                                                                                                           -` 1.3%
- `[unknown] [k] 0xffffffff8f2cf0e0                                                                                                           -` 1.12%

## board: CPU by shared object

- libgtk-4.so.1.2400.1 **54.56%**
- libc.so.6 **37.33%**
- libfontconfig.so.1.17.0 **2.85%**
- libpango-1.0.so.0.5800.2 **2.4%**
- relay-native **0.87%**
- libfreetype.so.6.20.6 **0.74%**
- libgobject-2.0.so.0.9000.1 **0.71%**

### board: hottest symbols

- `libgtk-4.so.1.2400.1 [.] 0x000000000039de88                                                                                                 ` 53.76%
- `libc.so.6 [.] 0x00000000000bcd10                                                                                                            ` 36.65%
- `libpango-1.0.so.0.5800.2 [.] 0x0000000000026c1c                                                                                             ` 1.23%
- `relay-native [.] <&mut serde_json::de::Deserializer<serde_json::read::SliceRead> as serde_core::de::Deserializer>::deserialize_struct::<<rel` 0.86%
- `libfontconfig.so.1.17.0 [.] 0x0000000000025cc0                                                                                              ` 0.83%
- `libgtk-4.so.1.2400.1 [.] gtk_widget_get_ancestor                                                                                            ` 0.81%
- `libfontconfig.so.1.17.0 [.] 0x0000000000034b50                                                                                              ` 0.74%
- `libfreetype.so.6.20.6 [.] 0x000000000000965a                                                                                                ` 0.74%
- `libgobject-2.0.so.0.9000.1 [.] g_type_check_instance_is_a                                                                                   ` 0.71%
- `libfontconfig.so.1.17.0 [.] 0x0000000000029b86                                                                                              ` 0.66%
- `libc.so.6 [.] cfree                                                                                                                         ` 0.65%
- `libfontconfig.so.1.17.0 [.] 0x0000000000034b74                                                                                              ` 0.61%
- `libpango-1.0.so.0.5800.2 [.] pango_layout_set_font_description                                                                              ` 0.59%
- `libpango-1.0.so.0.5800.2 [.] 0x0000000000027785                                                                                             ` 0.58%

## code: CPU by shared object

- libgtk-4.so.1.2400.1 **80.37%**
- libfontconfig.so.1.17.0 **7.21%**
- libglib-2.0.so.0.9000.1 **6.44%**
- libgobject-2.0.so.0.9000.1 **2.18%**
- libpango-1.0.so.0.5800.2 **2.14%**
- libc.so.6 **1.35%**

### code: hottest symbols

- `libgtk-4.so.1.2400.1 [.] 0x0000000000109b7e                                                                                                 ` 74.36%
- `libglib-2.0.so.0.9000.1 [.] g_datalist_id_dup_data                                                                                          ` 4.61%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b2439                                                                                                 ` 3.35%
- `libgtk-4.so.1.2400.1 [.] 0x00000000005c79b5                                                                                                 ` 2.65%
- `libfontconfig.so.1.17.0 [.] 0x0000000000029c90                                                                                              ` 2.47%
- `libfontconfig.so.1.17.0 [.] 0x0000000000029a91                                                                                              ` 2.4%
- `libfontconfig.so.1.17.0 [.] 0x000000000000a4d0                                                                                              ` 2.24%
- `libgobject-2.0.so.0.9000.1 [.] g_object_ref                                                                                                 ` 2.18%
- `libpango-1.0.so.0.5800.2 [.] 0x0000000000027d62                                                                                             ` 2.14%
- `libglib-2.0.so.0.9000.1 [.] g_list_prepend                                                                                                  ` 1.8%
- `libc.so.6 [.] cfree                                                                                                                         ` 1.35%

## notes: CPU by shared object

- libgobject-2.0.so.0.9000.1 **92.52%**
- libgtk-4.so.1.2400.1 **4.36%**
- libglib-2.0.so.0.9000.1 **3.03%**

### notes: hottest symbols

- `libgobject-2.0.so.0.9000.1 [.] g_type_is_a          -      -` 92.5%
- `libglib-2.0.so.0.9000.1 [.] g_hash_table_lookup  -      -` 1.89%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000397528   -      -` 1.62%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039df57   -      -` 1.54%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000358bf0   -      -` 1.16%
- `libglib-2.0.so.0.9000.1 [.] g_str_hash           -      -` 1.14%

## modules: CPU by shared object

- libfontconfig.so.1.17.0 **55.59%**
- libglib-2.0.so.0.9000.1 **23.06%**
- libgtk-4.so.1.2400.1 **18.17%**
- [unknown] **2.55%**

### modules: hottest symbols

- `libfontconfig.so.1.17.0 [.] 0x0000000000034bd0       -      -` 29.67%
- `libfontconfig.so.1.17.0 [.] 0x000000000002f372       -      -` 25.6%
- `libglib-2.0.so.0.9000.1 [.] 0x000000000006cc19       -      -` 23.06%
- `libgtk-4.so.1.2400.1 [.] gtk_accessible_get_type  -      -` 18.17%
- `[unknown] [k] 0xffffffff8ed7b90f       -      -` 1.07%

## settings: CPU by shared object

- libgtk-4.so.1.2400.1 **45.67%**
- libvulkan_radeon.so **20.66%**
- libgobject-2.0.so.0.9000.1 **15.18%**
- libglib-2.0.so.0.9000.1 **12.41%**
- libfontconfig.so.1.17.0 **3.65%**
- [unknown] **1.72%**

### settings: hottest symbols

- `libvulkan_radeon.so [.] 0x00000000004d4323                                                                                                  ` 20.65%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039df88                                                                                                 ` 17.0%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039de88                                                                                                 ` 16.35%
- `libgobject-2.0.so.0.9000.1 [.] g_enum_get_value_by_nick                                                                                     ` 14.73%
- `libglib-2.0.so.0.9000.1 [.] 0x00000000000c378a                                                                                              ` 12.39%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003974b3                                                                                                 ` 10.43%
- `libfontconfig.so.1.17.0 [.] 0x0000000000034bf0                                                                                              ` 0.6%
- `[unknown] [k] 0xffffffff8eccd06b                                                                                                            ` 0.53%
- `libfontconfig.so.1.17.0 [.] 0x0000000000034bd0                                                                                              ` 0.51%

## skills: CPU by shared object

- libc.so.6 **31.4%**
- libgtk-4.so.1.2400.1 **23.16%**
- libgobject-2.0.so.0.9000.1 **19.7%**
- [unknown] **13.06%**
- libglib-2.0.so.0.9000.1 **11.98%**

### skills: hottest symbols

- `libc.so.6 [.] 0x00000000000f25e0                                                                                                            ` 14.13%
- `libglib-2.0.so.0.9000.1 [.] g_rw_lock_writer_lock                                                                                           ` 11.51%
- `libc.so.6 [.] 0x000000000006b614                                                                                                            ` 11.38%
- `libgobject-2.0.so.0.9000.1 [.] 0x000000000003f425                                                                                           ` 10.38%
- `libgobject-2.0.so.0.9000.1 [.] g_type_is_a                                                                                                  ` 9.2%
- `[unknown] [k] 0xffffffff8ed0c609                                                                                                            ` 8.19%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000386ae1                                                                                                 ` 6.87%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000386ac2                                                                                                 ` 6.69%
- `libc.so.6 [.] 0x000000000005ef8a                                                                                                            ` 5.58%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000606b00                                                                                                 ` 4.95%
- `[unknown] [k] 0xffffffff8ecfbde5                                                                                                            ` 4.48%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003cfbdb                                                                                                 ` 3.4%

## dashboard: CPU by shared object

- libglib-2.0.so.0.9000.1 **48.64%**
- libgtk-4.so.1.2400.1 **34.92%**
- relay-native **11.02%**
- [unknown] **4.5%**
- libgobject-2.0.so.0.9000.1 **0.7%**

### dashboard: hottest symbols

- `libglib-2.0.so.0.9000.1 [.] g_queue_unlink                                -      -` 14.34%
- `relay-native [.] __rustc::__rust_no_alloc_shim_is_unstable_v2  -      -` 11.02%
- `libgtk-4.so.1.2400.1 [.] gtk_widget_show                               -      -` 10.13%
- `libgtk-4.so.1.2400.1 [.] gdk_frame_timings_get_frame_time              -      -` 9.05%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b66d7                            -      -` 9.03%
- `libglib-2.0.so.0.9000.1 [.] g_source_set_priority                         -      -` 8.33%
- `libglib-2.0.so.0.9000.1 [.] g_source_set_callback                         -      -` 7.96%
- `libglib-2.0.so.0.9000.1 [.] g_source_set_static_name                      -      -` 6.58%
- `libglib-2.0.so.0.9000.1 [.] 0x000000000006b4e4                            -      -` 6.13%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b2434                            -      -` 5.61%
- `libglib-2.0.so.0.9000.1 [.] g_hash_table_steal_extended                   -      -` 4.64%
- `[unknown] [k] 0xffffffff90029824                            -      -` 4.46%

## notifications: CPU by shared object

- libgtk-4.so.1.2400.1 **55.73%**
- libgobject-2.0.so.0.9000.1 **14.4%**
- libc.so.6 **12.3%**
- [unknown] **8.05%**
- libglib-2.0.so.0.9000.1 **5.19%**
- libgraphene-1.0.so.0.1000.8 **3.93%**

### notifications: hottest symbols

- `libgtk-4.so.1.2400.1 [.] 0x000000000020a514                                                                                                 ` 15.25%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b2430                                                                                                 ` 10.86%
- `libgobject-2.0.so.0.9000.1 [.] 0x000000000000f504                                                                                           ` 8.11%
- `libc.so.6 [.] 0x00000000001b8344                                                                                                            ` 6.19%
- `libgtk-4.so.1.2400.1 [.] gtk_widget_snapshot_child                                                                                          ` 6.12%
- `libgobject-2.0.so.0.9000.1 [.] g_type_check_instance_is_a                                                                                   ` 6.06%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b66a6                                                                                                 ` 5.78%
- `libgtk-4.so.1.2400.1 [.] 0x00000000006eefac                                                                                                 ` 5.77%
- `[unknown] [k] 0xffffffff8f2c8b24                                                                                                            ` 5.19%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003915c1                                                                                                 ` 4.56%
- `libgtk-4.so.1.2400.1 [.] 0x000000000035c4da                                                                                                 ` 4.25%
- `libgraphene-1.0.so.0.1000.8 [.] graphene_matrix_multiply                                                                                    ` 3.93%
- `libc.so.6 [.] 0x00000000000bc847                                                                                                            ` 3.08%
- `libc.so.6 [.] 0x00000000001b8a47                                                                                                            ` 2.91%
- `libglib-2.0.so.0.9000.1 [.] g_datalist_id_dup_data                                                                                          ` 2.59%
- `libgtk-4.so.1.2400.1 [.] 0x00000000005dafc4                                                                                                 ` 2.41%
- `[unknown] [k] 0xffffffff8ed16854                                                                                                            ` 2.41%
- `libglib-2.0.so.0.9000.1 [.] g_source_ref                                                                                                    ` 2.34%

## devices: CPU by shared object

- libglib-2.0.so.0.9000.1 **36.63%**
- libgtk-4.so.1.2400.1 **29.22%**
- libgobject-2.0.so.0.9000.1 **11.17%**
- libc.so.6 **9.74%**
- libvulkan_radeon.so **4.49%**
- libwayland-client.so.0.26.0 **4.0%**
- [unknown] **3.82%**
- libfontconfig.so.1.17.0 **0.55%**

### devices: hottest symbols

- `libgobject-2.0.so.0.9000.1 [.] g_closure_ref                                                         -      -` 10.81%
- `libglib-2.0.so.0.9000.1 [.] g_utf8_get_char_validated                                             -      -` 8.89%
- `libglib-2.0.so.0.9000.1 [.] g_source_set_priority                                                 -      -` 8.18%
- `libglib-2.0.so.0.9000.1 [.] g_slist_foreach                                                       -      -` 7.01%
- `libc.so.6 [.] pthread_mutex_unlock                                                  -      -` 6.74%
- `libglib-2.0.so.0.9000.1 [.] 0x000000000003f444                                                    -      -` 6.54%
- `libglib-2.0.so.0.9000.1 [.] g_free                                                                -      -` 5.48%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b96e3                                                    -      -` 4.9%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000387c46                                                    -      -` 4.71%
- `libvulkan_radeon.so [.] 0x000000000034ee54                                                    -      -` 4.49%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003ba0db                                                    -      -` 4.28%
- `libwayland-client.so.0.26.0 [.] wl_display_prepare_read                                               -      -` 4.0%
- `libgtk-4.so.1.2400.1 [.] gtk_widget_compute_transform                                          -      -` 3.88%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000391c30                                                    -      -` 3.88%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b9000                                                    -      -` 3.88%
- `[unknown] [k] 0xffffffff8eec0ec0                                                    -      -` 3.69%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b2439                                                    -      -` 2.88%
- `libc.so.6 [.] 0x00000000000fb466                                                    -      -` 2.73%

## launch-preview: CPU by shared object

- libgtk-4.so.1.2400.1 **69.48%**
- libc.so.6 **16.59%**
- libfreetype.so.6.20.6 **10.33%**
- libfontconfig.so.1.17.0 **1.87%**
- [unknown] **0.6%**
- libpango-1.0.so.0.5800.2 **0.57%**
- libgio-2.0.so.0.9000.1 **0.52%**

### launch-preview: hottest symbols

- `libgtk-4.so.1.2400.1 [.] 0x000000000039df53     -      -` 23.68%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000387ff4     -      -` 19.2%
- `libc.so.6 [.] cfree                  -      -` 16.28%
- `libgtk-4.so.1.2400.1 [.] 0x000000000038b226     -      -` 12.82%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000261bf7     -      -` 10.98%
- `libfreetype.so.6.20.6 [.] 0x0000000000087130     -      -` 10.33%
- `libfontconfig.so.1.17.0 [.] 0x0000000000029aae     -      -` 0.67%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039df60     -      -` 0.62%
- `libfontconfig.so.1.17.0 [.] 0x0000000000029632     -      -` 0.61%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039737b     -      -` 0.61%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039df39     -      -` 0.59%
- `libfontconfig.so.1.17.0 [.] 0x0000000000034b7f     -      -` 0.58%
- `libpango-1.0.so.0.5800.2 [.] 0x000000000003f840     -      -` 0.57%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000353740     -      -` 0.53%
- `libgio-2.0.so.0.9000.1 [.] g_icon_get_type        -      -` 0.52%
- `[unknown] [k] 0xffffffff900407bb     -      -` 0.51%

## palette: CPU by shared object

- libgtk-4.so.1.2400.1 **38.8%**
- libvulkan_radeon.so **21.98%**
- libc.so.6 **10.52%**
- [unknown] **8.91%**
- libwayland-client.so.0.26.0 **8.73%**
- libglib-2.0.so.0.9000.1 **4.5%**
- libgobject-2.0.so.0.9000.1 **4.3%**
- libpixman-1.so.0.46.4 **2.13%**

### palette: hottest symbols

- `libgtk-4.so.1.2400.1 [.] 0x000000000074ed92                  -      -` 7.51%
- `libc.so.6 [.] pthread_setspecific                 -      -` 6.69%
- `libgtk-4.so.1.2400.1 [.] 0x00000000006da1d4                  -      -` 6.62%
- `libgtk-4.so.1.2400.1 [.] gtk_overlay_get_type                -      -` 6.33%
- `libwayland-client.so.0.26.0 [.] 0x00000000000049e7                  -      -` 6.26%
- `libgtk-4.so.1.2400.1 [.] gtk_settings_get_for_display        -      -` 6.23%
- `libgtk-4.so.1.2400.1 [.] gsk_render_node_get_type            -      -` 5.0%
- `libvulkan_radeon.so [.] 0x00000000000de844                  -      -` 4.8%
- `libglib-2.0.so.0.9000.1 [.] g_source_attach                     -      -` 4.18%
- `libgobject-2.0.so.0.9000.1 [.] g_value_copy                        -      -` 4.08%
- `libvulkan_radeon.so [.] 0x00000000001dbb34                  -      -` 3.42%
- `libgtk-4.so.1.2400.1 [.] 0x000000000073a8f0                  -      -` 3.3%
- `libvulkan_radeon.so [.] 0x0000000000038d48                  -      -` 3.11%
- `libvulkan_radeon.so [.] 0x00000000001a0dc0                  -      -` 2.85%
- `libvulkan_radeon.so [.] 0x00000000000d934d                  -      -` 2.75%
- `libwayland-client.so.0.26.0 [.] wl_display_read_events              -      -` 2.47%
- `libc.so.6 [.] 0x00000000000fc9cc                  -      -` 2.37%
- `[unknown] [k] 0xffffffff8ecd440e                  -      -` 2.37%
- `libpixman-1.so.0.46.4 [.] 0x0000000000127212                  -      -` 2.13%
- `libvulkan_radeon.so [.] 0x00000000000d894a                  -      -` 2.01%

## layouts: CPU by shared object

- libpango-1.0.so.0.5800.2 **63.83%**
- [unknown] **31.13%**
- libgtk-4.so.1.2400.1 **2.5%**
- libvulkan_radeon.so **0.84%**
- libgio-2.0.so.0.9000.1 **0.81%**
- libc.so.6 **0.69%**

### layouts: hottest symbols

- `libpango-1.0.so.0.5800.2 [.] pango_extents_to_pixels                    -      -` 63.83%
- `[unknown] [k] 0xffffffff8ece874f                         -      -` 30.97%
- `libgtk-4.so.1.2400.1 [.] 0x000000000039e0fc                         -      -` 1.09%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003974a7                         -      -` 0.88%
- `libvulkan_radeon.so [.] 0x00000000003fe53c                         -      -` 0.84%
- `libgio-2.0.so.0.9000.1 [.] 0x00000000000d228e                         -      -` 0.81%
- `libc.so.6 [.] 0x00000000001b12e8                         -      -` 0.69%
- `libgtk-4.so.1.2400.1 [.] 0x00000000003b4c54                         -      -` 0.51%

## agents-streaming: CPU by shared object

- libgtk-4.so.1.2400.1 **28.07%**
- libc.so.6 **17.86%**
- libvte-2.91-gtk4.so.0 **17.26%**
- libgobject-2.0.so.0.9000.1 **15.2%**
- libpixman-1.so.0.46.4 **6.65%**
- libglib-2.0.so.0.9000.1 **6.55%**
- [unknown] **3.74%**
- libvulkan_radeon.so **2.6%**
- relay-native **1.82%**

### agents-streaming: hottest symbols

- `libgobject-2.0.so.0.9000.1 [.] 0x000000000003ee71                                                                                           ` 7.9%
- `libgtk-4.so.1.2400.1 [.] gsk_transform_node_new                                                                                             ` 7.74%
- `libc.so.6 [.] 0x00000000000efefd                                                                                                            ` 7.57%
- `libpixman-1.so.0.46.4 [.] 0x0000000000125e9e                                                                                                ` 6.65%
- `libc.so.6 [.] __libc_calloc                                                                                                                 ` 6.38%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000265f14                                                                                                 ` 6.28%
- `libvte-2.91-gtk4.so.0 [.] 0x00000000000b0974                                                                                                ` 5.38%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000795e2e                                                                                                 ` 5.21%
- `libvte-2.91-gtk4.so.0 [.] 0x00000000000aad85                                                                                                ` 4.62%
- `libvte-2.91-gtk4.so.0 [.] 0x000000000010c500                                                                                                ` 4.01%
- `libgobject-2.0.so.0.9000.1 [.] 0x000000000004020b                                                                                           ` 3.92%
- `libgtk-4.so.1.2400.1 [.] 0x000000000077a24b                                                                                                 ` 3.89%
- `libc.so.6 [.] 0x00000000000f4fbb                                                                                                            ` 3.51%
- `libglib-2.0.so.0.9000.1 [.] g_utf8_get_char_validated                                                                                       ` 3.38%
- `libgtk-4.so.1.2400.1 [.] 0x0000000000715b43                                                                                                 ` 3.34%
- `libglib-2.0.so.0.9000.1 [.] g_list_prepend                                                                                                  ` 3.14%
- `libgobject-2.0.so.0.9000.1 [.] 0x0000000000021178                                                                                           ` 3.05%
- `libvte-2.91-gtk4.so.0 [.] 0x000000000010e7b8                                                                                                ` 3.03%
- `libvulkan_radeon.so [.] 0x000000000015d6e1                                                                                                  ` 2.44%
- `[unknown] [k] 0xffffffff8ecfbe76                                                                                                            ` 2.15%

## agents-burst: CPU by shared object

- libgtk-4.so.1.2400.1 **28.37%**
- libglib-2.0.so.0.9000.1 **25.35%**
- relay-native **12.2%**
- libvulkan_radeon.so **11.84%**
- libvte-2.91-gtk4.so.0 **10.8%**
- libgobject-2.0.so.0.9000.1 **9.08%**
- [unknown] **2.12%**

### agents-burst: hottest symbols

- `libglib-2.0.so.0.9000.1 [.] g_str_hash                                                                                                      ` 24.78%
- `libvulkan_radeon.so [.] 0x00000000000cf120                                                                                                  ` 11.84%
- `libgtk-4.so.1.2400.1 [.] gtk_widget_get_ancestor                                                                                            ` 10.7%
- `libvte-2.91-gtk4.so.0 [.] 0x000000000007b313                                                                                                ` 10.52%
- `libgobject-2.0.so.0.9000.1 [.] g_signal_emit                                                                                                ` 9.08%
- `relay-native [.] <base64::engine::general_purpose::GeneralPurpose as base64::engine::Engine>::internal_decode                               ` 8.19%
- `libgtk-4.so.1.2400.1 [.] 0x000000000035811a                                                                                                 ` 7.25%
- `libgtk-4.so.1.2400.1 [.] gtk_native_get_surface                                                                                             ` 5.65%
- `libgtk-4.so.1.2400.1 [.] 0x000000000078ce84                                                                                                 ` 4.78%
- `relay-native [.] <relay_native::client::Client>::open_with_limit::{closure#0}::{closure#1}                                                  ` 1.13%
- `[unknown] [k] 0xffffffff8f2cf89e                                                                                                            ` 0.67%
