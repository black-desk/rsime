# SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
#
# SPDX-License-Identifier: MIT

vcpkg_from_github(
  OUT_SOURCE_PATH SOURCE_PATH
  REPO            rime/librime
  REF             1.15.0
  SHA512          9f808cc8dfe462a9076b2feb01acbd18505f23438757bd7f2efca9226c8115ce5c5852a70cae0abe85ecaea2a11b4944c8d4a1c06965af4bed5cd0274a07c369
  PATCHES         0001-Remove-FindIconv.cmake.patch)

vcpkg_from_github(
  OUT_SOURCE_PATH SOURCE_PATH_LUA
  REPO            hchunhui/librime-lua
  REF             68f9c364a2d25a04c7d4794981d7c796b05ab627
  SHA512          61702104890f7d5fa97e6cf05a6935e87c584efc855caac4c4611c939a97868b57909c59cd7b634800a124171ef6037bfffa860125c6f7cc1258f520ed583dcb
  PATCHES         0001-Find-lua-with-cmake.patch)

file(REMOVE_RECURSE "${SOURCE_PATH}/plugins/librime-lua")
file(RENAME "${SOURCE_PATH_LUA}" "${SOURCE_PATH}/plugins/librime-lua")

vcpkg_from_github(
  OUT_SOURCE_PATH SOURCE_PATH_OCTAGRAM
  REPO            lotem/librime-octagram
  REF             dfcc15115788c828d9dd7b4bff68067d3ce2ffb8
  SHA512          a8301e8141c85550790ac652a64d216154ee102a0d23c3965945cae408acf4b7c0d49876e999bbacdcfbfecd7988d395177830c798f919da9cdcaeeaed9db718)

file(REMOVE_RECURSE "${SOURCE_PATH}/plugins/librime-octagram")
file(RENAME "${SOURCE_PATH_OCTAGRAM}" "${SOURCE_PATH}/plugins/librime-octagram")

vcpkg_from_github(
  OUT_SOURCE_PATH SOURCE_PATH_PREDICT
  REPO            rime/librime-predict
  REF             920bd41ebf6f9bf6855d14fbe80212e54e749791
  SHA512          8d4517ff1cfaf8b73b5a56e36e683db0db582c19e72623f3a1c94c165a25477a9f42ef13b575974e9c00ab376417aeef7333f57d7783a9ac91ad8bd5028700eb)

file(REMOVE_RECURSE "${SOURCE_PATH}/plugins/librime-predict")
file(RENAME "${SOURCE_PATH_PREDICT}" "${SOURCE_PATH}/plugins/librime-predict")

vcpkg_cmake_configure(
  SOURCE_PATH "${SOURCE_PATH}"
  OPTIONS
    -DBUILD_TEST=OFF)

vcpkg_cmake_install()

vcpkg_cmake_config_fixup(
  PACKAGE_NAME  "rime"
  CONFIG_PATH   "share/cmake/rime"
)

vcpkg_fixup_pkgconfig()

file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")

vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE")
