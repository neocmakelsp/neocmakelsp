cmake_minimum_required(VERSION 3.13)

add_subdirectory(subdir)
find_package(DtkGui)
find_package(ECM)
include(./test1.cmake)
include(./test3.cmake)
include(./DWayland.cmake)
