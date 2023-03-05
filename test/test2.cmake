cmake_minimum_required(VERSION 3.16)
project(states
  LANGUAGES CXX
)
set(Acc 1)
set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
if(NOT DEFINED INSTALL_EXAMPLESDIR)
  set(INSTALL_EXAMPLESDIR "examples")
endif()
set(INSTALL_EXAMPLEDIR "${INSTALL_EXAMPLESDIR}/widgets/animation/states")
find_package(Qt6 REQUIRED COMPONENTS
	Core Gui StateMachine Widgets)
qt_add_executable(states main.cpp)
set_target_properties(states PROPERTIES
  WIN32_EXECUTABLE TRUE
  MACOSX_BUNDLE TRUE)
target_link_libraries(states PUBLIC
  Qt::Core
  Qt::Gui
  Qt::StateMachine
  Qt::Widgets
  Qt6::StateMachine)
# Resources:
set(states_resource_files
  "accessories-dictionary.png"
  "akregator.png"
  "digikam.png"
  "help-browser.png"
  "k3b.png"
  "kchart.png"
)
qt6_add_resources(states "states"
  PREFIX
  "/"
  FILES
  ${states_resource_files})
install(TARGETS states
  RUNTIME DESTINATION "${INSTALL_EXAMPLEDIR}"
  BUNDLE DESTINATION "${INSTALL_EXAMPLEDIR}"
  LIBRARY DESTINATION "${INSTALL_EXAMPLEDIR}")
