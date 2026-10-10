vcpkg_check_linkage(ONLY_DYNAMIC_LIBRARY)
vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/sekrit-twc/zimg/archive/refs/tags/release-3.0.5.tar.gz"
    FILENAME "zimg-3.0.5.tar.gz"
    SHA512 85467ec9fcf81ea1ae3489b539ce751772a1dab6c6159928b3c5aa9f1cb029f0570b4624a254d4620886f3376fbf80f9bb829a88c3fe543f99f38947951dc500)
vcpkg_extract_source_archive(SOURCE_PATH ARCHIVE "${ARCHIVE}")
# Use the reviewed upstream MSVC projects, including their per-file SIMD flags.
# The DLL links the upstream static intermediate; only its import library is installed.
set(OUTPUT "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel/output/")
vcpkg_build_msbuild(PROJECT_PATH "${SOURCE_PATH}/_msvc/zimg/zimg.vcxproj"
    PLATFORM x64 OPTIONS_RELEASE "/p:OutDir=${OUTPUT}" "/p:IntDir=${OUTPUT}obj-core/")
vcpkg_build_msbuild(PROJECT_PATH "${SOURCE_PATH}/_msvc/dll/dll.vcxproj"
    PLATFORM x64 OPTIONS_RELEASE "/p:OutDir=${OUTPUT}" "/p:IntDir=${OUTPUT}obj-dll/")
file(INSTALL "${OUTPUT}z.dll" DESTINATION "${CURRENT_PACKAGES_DIR}/bin")
file(INSTALL "${OUTPUT}z_imp.lib" DESTINATION "${CURRENT_PACKAGES_DIR}/lib" RENAME zimg.lib)
file(INSTALL "${SOURCE_PATH}/src/zimg/api/zimg.h" DESTINATION "${CURRENT_PACKAGES_DIR}/include")
file(MAKE_DIRECTORY "${CURRENT_PACKAGES_DIR}/lib/pkgconfig")
file(WRITE "${CURRENT_PACKAGES_DIR}/lib/pkgconfig/zimg.pc"
"prefix=\${pcfiledir}/../..\nlibdir=\${prefix}/lib\nincludedir=\${prefix}/include\nName: zimg\nDescription: Scaling and colorspace conversion\nVersion: 3.0.5\nLibs: -L\${libdir} -lzimg\nCflags: -I\${includedir}\n")
vcpkg_fixup_pkgconfig()
file(INSTALL "${SOURCE_PATH}/COPYING" DESTINATION "${CURRENT_PACKAGES_DIR}/share/${PORT}" RENAME copyright)
