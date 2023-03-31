cmake_minimum_required(VERSION 3.13)

add_subdirectory(subdir)
find_package(DtkGui)
find_package(ECM)
include(./test1.cmake)
include(./test3.cmake)
include(./DWayland.cmake)
pkg_check_modules(libva REQUIRED IMPORTED_TARGET libsystemd)
find_package(Qt${QT_VERSION_MAJOR} REQUIRED COMPONENTS Core)

# Not Format Me
target_link_libraries(ss PUBLIC
	${DtkGui_LIBRARIES}
	PkgConfig::libevent_core
)

# Not Format Me
target_include_directories(ss PUBLIC
	${DtkGui_INCLUDE_DIRS}
)

if(A)
	target_link_libraries(ss PUBLIC
		${DtkGui_LIBRARIES}
		PkgConfig::libevent_core)


	# Not Format Me
	target_include_directories(ss PUBLIC
		${DtkGui_INCLUDE_DIRS}
	)
endif()

foreach()
	target_link_libraries(ss PUBLIC
		${DtkGui_LIBRARIES}
		PkgConfig::libevent_core)
	# Not Format Me
	target_include_directories(ss PUBLIC
		${DtkGui_INCLUDE_DIRS}
	)
endforeach()
