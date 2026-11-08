cmake_minimum_required(VERSION 3.28.0)

if(BOARD MATCHES "^esp32p4_function_ev_board(/|$)")
  execute_process(COMMAND west list zephyr-lang-rust -f "{abspath}"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    OUTPUT_VARIABLE rust_module_source OUTPUT_STRIP_TRAILING_WHITESPACE
    RESULT_VARIABLE rust_module_result)
  if(NOT rust_module_result EQUAL 0 OR NOT EXISTS "${rust_module_source}/zephyr/module.yml")
    message(FATAL_ERROR "Unable to locate the Zephyr Rust module")
  endif()
  set(rust_module_copy "${CMAKE_CURRENT_BINARY_DIR}/pocketgen-rust-module")
  file(COPY "${rust_module_source}/" DESTINATION "${rust_module_copy}" PATTERN ".git" EXCLUDE)
  set(generator "${rust_module_copy}/zephyr-build/src/lib.rs")
  file(READ "${generator}" generator_source)
  set(original [=[writeln!(&mut f, "pub const {}: usize = {};", &caps[1], &caps[2]).unwrap();]=])
  set(replacement [=[let value = u64::from_str_radix(&caps[2][2..], 16).expect("invalid Kconfig hex value");
            let width = env::var("CARGO_CFG_TARGET_POINTER_WIDTH").expect("target pointer width missing");
            let ty = if width == "32" && value > u32::MAX as u64 { "u64" } else { "usize" };
            writeln!(&mut f, "pub const {}: {} = {};", &caps[1], ty, &caps[2]).unwrap();]=])
  string(FIND "${generator_source}" "${original}" patch_position)
  if(patch_position EQUAL -1)
    message(FATAL_ERROR "Unsupported Zephyr Rust Kconfig generator")
  endif()
  string(REPLACE "${original}" "${replacement}" generator_source "${generator_source}")
  file(WRITE "${generator}" "${generator_source}")
  set(rust_cmake "${rust_module_copy}/CMakeLists.txt")
  file(READ "${rust_cmake}" rust_cmake_source)
  set(original [=[set(RUST_TARGET "riscv32i-unknown-none-elf" PARENT_SCOPE)]=])
  set(replacement [=[if(NOT CONFIG_FPU)
        message(FATAL_ERROR "ESP32-P4 requires CONFIG_FPU=y for the Rust hard-float ABI")
      endif()
      set(RUST_TARGET "riscv32imafc-unknown-none-elf" PARENT_SCOPE)]=])
  string(FIND "${rust_cmake_source}" "${replacement}" patch_position)
  if(patch_position EQUAL -1)
    string(FIND "${rust_cmake_source}" "${original}" patch_position)
    if(patch_position EQUAL -1)
      message(FATAL_ERROR "Unsupported Zephyr Rust RISC-V target mapping")
    endif()
    string(REPLACE "${original}" "${replacement}" rust_cmake_source "${rust_cmake_source}")
    file(WRITE "${rust_cmake}" "${rust_cmake_source}")
  endif()
  set(rust_allocator "${rust_module_copy}/zephyr/src/alloc_impl.rs")
  file(READ "${rust_allocator}" rust_allocator_source)
  set(original [=[#[global_allocator]
pub static ZEPHYR_ALLOCATOR: ZephyrAllocator = ZephyrAllocator;]=])
  set(replacement [=[#[cfg_attr(not(CONFIG_SOC_ESP32P4), global_allocator)]
pub static ZEPHYR_ALLOCATOR: ZephyrAllocator = ZephyrAllocator;]=])
  string(FIND "${rust_allocator_source}" "${replacement}" patch_position)
  if(patch_position EQUAL -1)
    string(FIND "${rust_allocator_source}" "${original}" patch_position)
    if(patch_position EQUAL -1)
      message(FATAL_ERROR "Unsupported Zephyr Rust global allocator")
    endif()
    string(REPLACE "${original}" "${replacement}" rust_allocator_source "${rust_allocator_source}")
    file(WRITE "${rust_allocator}" "${rust_allocator_source}")
  endif()
  list(APPEND EXTRA_ZEPHYR_MODULES "${rust_module_copy}")
endif()

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(pocketgen)

if(NOT BOARD STREQUAL "native_sim" AND NOT BOARD STREQUAL "esp32p4_function_ev_board")
  message(FATAL_ERROR "The display and MicroPython integration supports native_sim and esp32p4_function_ev_board only")
endif()

set(MICROPY_DIR ${CMAKE_CURRENT_SOURCE_DIR}/components/micropython)
set(MICROPY_TARGET micropython)
set(MICROPY_PORT_DIR ${CMAKE_CURRENT_SOURCE_DIR}/src/python)
set(MICROPY_FROZEN_MANIFEST ${CMAKE_CURRENT_SOURCE_DIR}/src/python/manifest.py)
set(MICROPY_MPY_TOOL_FLAGS "-mlongint-impl=none")
include(${MICROPY_DIR}/py/py.cmake)
include(${MICROPY_DIR}/py/manifest.cmake)

set(MICROPY_SOURCE_QSTR
  ${MICROPY_SOURCE_PY}
  ${MICROPY_DIR}/shared/runtime/gchelper_generic.c
  ${CMAKE_CURRENT_SOURCE_DIR}/src/c/micropython.c
)
set(MICROPY_QSTRDEFS_PORT ${CMAKE_CURRENT_SOURCE_DIR}/src/c/qstrdefsport.h)

zephyr_get_include_directories_for_lang(C includes)
zephyr_get_system_include_directories_for_lang(C system_includes)
zephyr_get_compile_definitions_for_lang(C definitions)
zephyr_get_compile_options_for_lang(C options)
set(MICROPY_CPP_FLAGS_EXTRA ${includes} ${system_includes} ${definitions} ${options})

zephyr_library_named(${MICROPY_TARGET})
zephyr_library_include_directories(
  ${MICROPY_DIR}
  ${CMAKE_CURRENT_SOURCE_DIR}/src/c
  ${CMAKE_CURRENT_BINARY_DIR}
)
zephyr_library_compile_options(-std=gnu99 -fomit-frame-pointer)
zephyr_library_compile_definitions(NDEBUG)
zephyr_library_sources(
  ${MICROPY_SOURCE_PY}
  ${MICROPY_DIR}/shared/runtime/gchelper_generic.c
)
zephyr_library_link_libraries(kernel)
# Resolve C math before the Rust target's compiler-builtins: their native-sim
# floating-point ABI differs from the host C ABI. Picolibc stores libm in libc.
if(BOARD STREQUAL "native_sim")
  target_sources(app PRIVATE src/c/native_math.c)
  target_sources(native_simulator INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/src/c/native_math_host.c)
  target_link_options(native_simulator INTERFACE "-lm")
else()
  zephyr_library_link_libraries(m c)
endif()
include(${MICROPY_DIR}/py/mkrules.cmake)
add_custom_command(OUTPUT ${MICROPY_QSTRDEFS_LAST} APPEND
  DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/src/c/mpconfigport.h
          ${CMAKE_CURRENT_SOURCE_DIR}/src/c/mphalport.h)
add_dependencies(BUILD_VERSION_HEADER zephyr_generated_headers)
add_dependencies(${MICROPY_TARGET} zephyr_generated_headers)
add_dependencies(app ${MICROPY_TARGET})
target_link_libraries(app PRIVATE ${MICROPY_TARGET})
target_include_directories(app PRIVATE ${MICROPY_DIR} ${CMAKE_CURRENT_SOURCE_DIR}/src/c
  ${CMAKE_CURRENT_BINARY_DIR})
target_sources(app PRIVATE src/c/display.c src/c/micropython.c src/c/input.c)
target_sources_ifdef(CONFIG_SOC_ESP32P4 app PRIVATE src/c/allocator.c)
target_sources_ifdef(CONFIG_POCKETGEN_SELF_TEST app PRIVATE tests/micropython.c)

rust_cargo_application()