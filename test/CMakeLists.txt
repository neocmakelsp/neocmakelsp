cmake_minimum_required(VERSION 3.13)

add_subdirectory(subdir)
find_package(DtkGui)
find_package(ECM)
include(./test1.cmake)
include(./test3.cmake)
include(./DWayland.cmake)
pkg_check_modules(libva REQUIRED IMPORTED_TARGET libsystemd)
target_link_libraries(ss PUBLIC
	${DtkGui_LIBRARIES}
	PkgConfig::libevent_core
)
target_include_directories(ss PUBLIC
	${DtkGui_INCLUDE_DIRS}
)
