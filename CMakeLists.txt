cmake_minimum_required(VERSION 3.10)
project(DonutEx)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(BUILD_SHARED_LIBS OFF)
set(CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE} -s")

set(IMGUI_SOURCES
    imgui/imgui.cpp
    imgui/imgui_draw.cpp
    imgui/imgui_tables.cpp
    imgui/imgui_widgets.cpp
    imgui/backends/imgui_impl_glfw.cpp
    imgui/backends/imgui_impl_opengl3.cpp
    imgui/misc/cpp/imgui_stdlib.cpp
)

set(GL3W_SOURCES
    gl3w/src/gl3w.c
)

set(MAIN_SOURCES
    main.cpp
    TextEditor.cpp
    LuaBindings.cpp
)

add_subdirectory(nativefiledialog-extended)

add_executable(DonutEx
    ${MAIN_SOURCES}
    ${IMGUI_SOURCES}
    ${GL3W_SOURCES}
)

target_include_directories(DonutEx PRIVATE
    lua
    imgui
    imgui/backends
    gl3w/include
    ${GLFW3_INCLUDE_DIRS}
    nativefiledialog-extended/src/include
    nanosvg/src
)

set(GLFW3_STATIC_LIBRARIES "${CMAKE_SOURCE_DIR}/glfw/build/src/libglfw3.a")
include_directories(${CMAKE_SOURCE_DIR}/glfw/include)

find_package(X11 REQUIRED)
target_link_libraries(DonutEx
    ${GLFW3_STATIC_LIBRARIES}
    ${CMAKE_SOURCE_DIR}/lua/liblua.a
    GL
    ${CMAKE_DL_LIBS}
    X11::X11
    nfd
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
