cmake_minimum_required(VERSION 3.16)

set(KF_VERSION "5.240.0") # handled by release scripts
project(KCoreAddons VERSION ${KF_VERSION})

include(FeatureSummary)
find_package(ECM 5.240.0  NO_MODULE)
set_package_properties(ECM PROPERTIES TYPE REQUIRED DESCRIPTION "Extra CMake Modules." URL "https://commits.kde.org/extra-cmake-modules")
feature_summary(WHAT REQUIRED_PACKAGES_NOT_FOUND FATAL_ON_MISSING_REQUIRED_PACKAGES)


set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH} ${CMAKE_CURRENT_SOURCE_DIR}/cmake)

include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDEFrameworkCompilerSettings NO_POLICY_SCOPE)
include(KDEGitCommitHooks)

include(ECMGenerateExportHeader)
include(CMakePackageConfigHelpers)
include(ECMSetupVersion)
include(ECMGenerateHeaders)
include(ECMQtDeclareLoggingCategory)
include(ECMAddQch)
include(ECMSetupQtPluginMacroNames)
include(ECMDeprecationSettings)
include(ECMQmlModule)

set(EXCLUDE_DEPRECATED_BEFORE_AND_AT 0 CACHE STRING "Control the range of deprecated API excluded from the build [default=0].")

option(BUILD_QCH "Build API documentation in QCH format (for e.g. Qt Assistant, Qt Creator & KDevelop)" OFF)
add_feature_info(QCH ${BUILD_QCH} "API documentation in QCH format (for e.g. Qt Assistant, Qt Creator & KDevelop)")

option(ENABLE_PCH "Enable precompile headers for faster builds" ON)
option(KCOREADDONS_USE_QML "Build the QML plugin" ON)

set(REQUIRED_QT_VERSION 6.5.0)
find_package(Qt6 ${REQUIRED_QT_VERSION} CONFIG REQUIRED Core)
if (KCOREADDONS_USE_QML)
    find_package(Qt6 ${REQUIRED_QT_VERSION} CONFIG REQUIRED Qml)
endif()

ecm_setup_qtplugin_macro_names(
    JSON_NONE
        "K_PLUGIN_FACTORY"
        "K_PLUGIN_CLASS"
    JSON_ARG2
        "K_PLUGIN_FACTORY_WITH_JSON"
        "K_PLUGIN_CLASS_WITH_JSON"
    CONFIG_CODE_VARIABLE
        PACKAGE_SETUP_AUTOMOC_VARIABLES
)

if(NOT WIN32)
    find_package(Threads REQUIRED)
endif()

# Configure checks for kdirwatch
find_package(FAM)

set_package_properties(FAM PROPERTIES
     PURPOSE "Provides file alteration notification facilities using a separate service. FAM provides additional support for NFS.")

set(HAVE_FAM ${FAM_FOUND})

option(ENABLE_INOTIFY "Try to use inotify for directory monitoring" ON)
if(ENABLE_INOTIFY)
    # Find libinotify
    find_package(Inotify)
    set_package_properties(Inotify PROPERTIES
        PURPOSE "Filesystem alteration notifications using inotify")
    set(HAVE_SYS_INOTIFY_H ${Inotify_FOUND})
else()
    set(HAVE_SYS_INOTIFY_H FALSE)
endif()

set(HAVE_PROCSTAT FALSE)
string(REGEX MATCH "[Bb][Ss][Dd]" BSDLIKE ${CMAKE_SYSTEM_NAME})
if (BSDLIKE)
    option(ENABLE_PROCSTAT "Try to use libprocstat for process information (for BSD-like systems)" ON)
    if (ENABLE_PROCSTAT)
        # Find libprocstat
        find_package(Procstat)
        set_package_properties(Procstat PROPERTIES
            PURPOSE "Process information using libprocstat")
        set(HAVE_PROCSTAT ${PROCSTAT_FOUND})
    endif()
    if (CMAKE_SYSTEM_NAME MATCHES "FreeBSD")
        set_package_properties(Procstat PROPERTIES
            TYPE REQUIRED
        )
    endif()
endif()

if(NOT WIN32) # never relevant there
    find_package(Qt6DBus ${QT_MIN_VERSION} CONFIG)
    set(HAVE_QTDBUS ${Qt6DBus_FOUND})
    add_feature_info(XDGPortalDragAndDrop HAVE_QTDBUS "Drag and Drop support via xdg-desktop-portal requies QtDBus")
endif()

if (CMAKE_SYSTEM_NAME MATCHES "Linux")
    find_package(UDev) # Used by KFilesystemType
    set(HAVE_UDEV ${UDev_FOUND})
endif()

configure_file(src/lib/io/config-kdirwatch.h.cmake ${CMAKE_CURRENT_BINARY_DIR}/src/lib/io/config-kdirwatch.h)

configure_file(src/lib/io/config-kfilesystemtype.h.cmake ${CMAKE_CURRENT_BINARY_DIR}/src/lib/io/config-kfilesystemtype.h)

include(ECMPoQmTools)

set(kcoreaddons_version_header "${CMAKE_CURRENT_BINARY_DIR}/src/lib/kcoreaddons_version.h")
ecm_setup_version(PROJECT VARIABLE_PREFIX KCOREADDONS
                        VERSION_HEADER "${kcoreaddons_version_header}"
                        PACKAGE_VERSION_FILE "${CMAKE_CURRENT_BINARY_DIR}/KF6CoreAddonsConfigVersion.cmake"
                        SOVERSION 6)


ecm_install_po_files_as_qm(poqm)

kde_enable_exceptions()


ecm_set_disabled_deprecation_versions(
    QT 6.5.0
)

add_subdirectory(src)
if (BUILD_TESTING)
    add_subdirectory(autotests)
    add_subdirectory(tests)
endif()

# create a Config.cmake and a ConfigVersion.cmake file and install them
set(CMAKECONFIG_INSTALL_DIR "${KDE_INSTALL_CMAKEPACKAGEDIR}/KF6CoreAddons")

if (BUILD_QCH)
    ecm_install_qch_export(
        TARGETS KF6CoreAddons_QCH
        FILE KF6CoreAddonsQchTargets.cmake
        DESTINATION "${CMAKECONFIG_INSTALL_DIR}"
        COMPONENT Devel
    )
    set(PACKAGE_INCLUDE_QCHTARGETS "include(\"\${CMAKE_CURRENT_LIST_DIR}/KF6CoreAddonsQchTargets.cmake\")")
endif()

configure_package_config_file("${CMAKE_CURRENT_SOURCE_DIR}/KF6CoreAddonsConfig.cmake.in"
                              "${CMAKE_CURRENT_BINARY_DIR}/KF6CoreAddonsConfig.cmake"
                              INSTALL_DESTINATION  ${CMAKECONFIG_INSTALL_DIR}
                              )

install(FILES  "${CMAKE_CURRENT_BINARY_DIR}/KF6CoreAddonsConfig.cmake"
               "${CMAKE_CURRENT_BINARY_DIR}/KF6CoreAddonsConfigVersion.cmake"
               "${CMAKE_CURRENT_SOURCE_DIR}/KF6CoreAddonsMacros.cmake"
        DESTINATION "${CMAKECONFIG_INSTALL_DIR}"
        COMPONENT Devel )

install(EXPORT KF6CoreAddonsTargets DESTINATION "${CMAKECONFIG_INSTALL_DIR}" FILE KF6CoreAddonsTargets.cmake NAMESPACE KF6:: )

install(FILES ${kcoreaddons_version_header} DESTINATION ${KDE_INSTALL_INCLUDEDIR_KF}/KCoreAddons COMPONENT Devel)

feature_summary(WHAT ALL FATAL_ON_MISSING_REQUIRED_PACKAGES)

kde_configure_git_pre_commit_hook(CHECKS CLANG_FORMAT)
