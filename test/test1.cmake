project(mygame)
cmake_minimum_required(VERSION 3.22)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
if(CMAKE_BUILD_TYPE STRLESS_EQUAL "Debug")
	set(STARTTEST ON)
endif ()
#add_executable(cmake_study src/main.cpp src/header.h)
add_subdirectory(src/game)
add_subdirectory(src/backend)
add_subdirectory(src/pingit)
if(AA
	AND CC)


	if(STARTTEST
		AND B)


		add_subdirectory(tests)
	endif ()
endif()
find_package(DtkCore)
include(./test2.cmake)
include(GNUInstallDirs)
add_definitions(-DAA="sss")
