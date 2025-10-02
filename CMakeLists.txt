cmake_minimum_required(VERSION 3.10)
project(DonutEx)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(BUILD_SHARED_LIBS OFF)
set(CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE} -s")

set(MAIN_SOURCES
    main.cpp
    TextEditor.cpp
    LuaBindings.cpp
    PieceTable.cpp
)

include_directories(
    /usr/local/include
    /usr/local/include/imgui
    /usr/local/include/imgui/backends
    /usr/local/include/gl3w
    lua
    nanosvg/src
)

find_library(GLFW3 glfw3 PATHS /usr/local/lib)
find_library(IMGUI imgui PATHS /usr/local/lib)
find_library(GL3W gl3w PATHS /usr/local/lib)
find_library(NFD nfd PATHS /usr/local/lib)

add_executable(DonutEx
    ${MAIN_SOURCES}
)

find_package(X11 REQUIRED)

find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK3 REQUIRED gtk+-3.0)

include_directories(${GTK3_INCLUDE_DIRS})
link_directories(${GTK3_LIBRARY_DIRS})
add_definitions(${GTK3_CFLAGS_OTHER})

target_link_libraries(DonutEx
    ${GLFW3}
    ${IMGUI}
    ${GL3W}
    ${NFD}
    ${GTK3_LIBRARIES}
    ${CMAKE_SOURCE_DIR}/lua/liblua.a
    GL
    ${CMAKE_DL_LIBS}
    X11::X11
)

install(TARGETS DonutEx
    RUNTIME DESTINATION bin
)

install(DIRECTORY plugins/
    DESTINATION plugins
    FILES_MATCHING PATTERN "*.lua"
)

install(DIRECTORY icons/
    DESTINATION icons
    FILES_MATCHING PATTERN "*.svg"
)
