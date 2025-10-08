# Neocmakelsp

## 0.8.30

- This version use the forked lsp-types, bump the dependence of
  fluent-uri. This release contains a lot of experiment things, like using
  the async trait of rust instead of that of async-trait, like using
  fluent-uri instead of the crate of url.

- This release also is used to
  test the pr for fluent-uri. I believe the fluent-uri is better than that
  url, and also make a pr, but the author these days is very busy, and
  does have time to review my pr, so I added my modification to my forked
  lsp-types which is named as lsp-types-f. If there is any problem, please
  open an issue for me, I will try to fix it

## 0.8.20-beta4
- fix that every time save a file, the references will increase, which also cause problems when doing rename

## 0.8.20-beta3
- fix when using relative cmake path, reference not work properly

## 0.8.20-beta2
- fix rename do not work for include
- fix rename not work if position is on definition

## 0.8.20-beta1
- support real reference
- support rename
- to edition 2024

## 0.8.8
- futures-util v0.3.30 is yanked, so publish new release

## 0.8.7
- Fix complete when meet comment panic on windows
- Better way to find the platform prefix thanks to @idealseal
- improve logging for stdio transport @idealseal
- rename buildin to builtin, typo fix
- bring the cli color of clap
- add LTO support by @zamazan4ik

## 0.8.5
- Add a lot of unit tests
- Fix that fileapi cache data cannot be updated.
- Realize the lsp document_link
- Make the hovered information the same as completion information
- Support completing with cmake space.
- Change the way generate the snippet
- Now the `insert_final_newline` action will work.
- Fix the meson cargo wrapper again. I think this time it is usable now.
- Tidy up a lot of code.
- Now it can jump to `"${SOME_VARIABLE}/some.cmake"` or `"some.cmake"`. It supports to read the variable.
- Adjust some document format

Full changes: https://github.com/neocmakelsp/neocmakelsp/compare/v0.8.4...v0.8.5

## 0.8.4
- Fix jump to buildin cmake file still not works on temux
- Try to support find_package on MSYSTEM
- Add some unit test. Now it is 30% coverage!
- Now hover and complete will show the comment of cmake

## 0.8.3
- support reading value from fileapi and use it in completing
- fix jumping to buildin cmake file not works on temux
- fix meson build, induce a python wrapper

## 0.8.1

- Compatible with vcpkg package manager

## 0.8.0

- support file api
- use lazylock
- support jump from function to files

## 0.7.6

- feat: Update CompletionItem to meet the requirements of the LSP specification, by yangyingchao
- add completiontions for fish, bash and etc
- Use derive for subcommand

## 0.7.5

- fix panic when meet pkg_check_modules thanks to @yangyingchao
- better performance, reduce too many collect
- fix too much typo
