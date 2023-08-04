option(ENABLE_INOTIFY "Try to use inotify for directory monitoring" ON)
#ss`
if(ENABLE_INOTIFY)

	# Find libinotify

	find_package(Inotify) # ss
	set_package_properties(Inotify PROPERTIES
									PURPOSE "Filesystem alteration notifications using inotify")
endif()
