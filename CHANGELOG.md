# Neocmakelsp

## 0.8.4
- Fix jump to buildin cmake file still not works on temux
- Try to support find_package on MSYSTEM
- Add some unit test. Now it is 30% coverage!
- Now hover and complete will show the comment of cmake

## 0.8.3
- support reading value from fileapi and use it in compeleting
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
