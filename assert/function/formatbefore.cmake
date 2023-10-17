# ss

function(A)

#ss
	set(A
		2)

	option(E e)

	set(MM
		A # test
		B
		C
		)

endfunction()

	if(ENABLE_INOTIFY) #ss #ss

	# Find libinotify

		find_package(Inotify) # ss
	set_package_properties(Inotify PROPERTIES #ss
									PURPOSE "Filesystem alteration notifications using inotify")
endif()

