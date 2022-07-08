if(NOT (CMAKE_BUILD_TYPE OR CMAKE_CONFIGURATION_TYPES))
		set(CMAKE_BUILD_TYPE "Debug" CACHE STRING
					"Choose the type of build, options are: None Debug Release RelWithDebInfo MinSizeRel."
							FORCE)
							message("Setting build type to '${CMAKE_BUILD_TYPE}'")
						else(NOT (CMAKE_BUILD_TYPE OR CMAKE_CONFIGURATION_TYPES))
								message("Build type set to '${CMAKE_BUILD_TYPE}'")
							endif(NOT (CMAKE_BUILD_TYPE OR CMAKE_CONFIGURATION_TYPES))
