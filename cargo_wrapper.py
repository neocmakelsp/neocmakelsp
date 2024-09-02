#!/usr/bin/python

import sys

import subprocess

import shutil

from pathlib import Path

build_type: str = sys.argv[1]

build_command = ["cargo", "build"]

if build_type == "release":
    build_command.append("--release")

source_path_str: str = sys.argv[2]

target_path_str: str = sys.argv[3]

subprocess.run(build_command, capture_output=False)

source_path = Path(source_path_str)

target_path = Path(target_path_str)

shutil.copy(source_path_str, target_path)
