function(DWaylandScannerClientHEAD name file_name ret)
	find_program(QWaylandScanner "qtwaylandscanner")
	find_program(WaylandScanner "wayland-scanner")
	if(WaylandScanner)
		message("-- Found Program WaylandScanner")
	else()
		message(FATAL_ERROR "Cannot find WaylandScanner")
	endif()
	set(qwaylandscanner "qtwaylandscanner")
	if(NOT QWaylandScanner)
		find_program(Qt5WaylandScanner "/usr/lib/qt5/bin/qtwaylandscanner")
		if (Qt5WaylandScanner)
			set(qtwaylandscanner "/usr/lib/qt5/bin/qtwaylandscanner")
		else()
			message(FATAL_ERROR "Cannot find QtWaylandScanner")
		endif()
	endif()
	message("-- Found QtWaylandScanner")
	execute_process(
		COMMAND ${qwaylandscanner} client-header ${file_name}
		OUTPUT_FILE ${CMAKE_CURRENT_BINARY_DIR}/qwayland-${name}.h)
	execute_process(
		COMMAND ${qwaylandscanner} client-code ${file_name}
		OUTPUT_FILE ${CMAKE_CURRENT_BINARY_DIR}/qwayland-${name}.cpp)
	execute_process(
		COMMAND wayland-scanner client-header ${file_name} ${CMAKE_CURRENT_BINARY_DIR}/wayland-${name}-client-protocol.h)
	execute_process(
		COMMAND wayland-scanner public-code ${file_name} ${CMAKE_CURRENT_BINARY_DIR}/wayland-${name}-protocol.c)
	# append new
	list(APPEND retn
		${CMAKE_CURRENT_BINARY_DIR}/qwayland-${name}.h
		${CMAKE_CURRENT_BINARY_DIR}/qwayland-${name}.cpp
		${CMAKE_CURRENT_BINARY_DIR}/wayland-${name}-client-protocol.h
		${CMAKE_CURRENT_BINARY_DIR}/wayland-${name}-protocol.c)
	set(${ret} ${retn} PARENT_SCOPE)
endfunction()
