#!/usr/bin/python

import sys

import subprocess

import shutil

import os

from pathlib import Path

source_dir: str = sys.argv[1]

os.chdir(source_dir)

build_type: str = sys.argv[2]

build_command = ["cargo", "build"]

if build_type == "release":
    build_command.append("--release")

source_path_str: str = sys.argv[3]

target_path_str: str = sys.argv[4]

subprocess.run(build_command, capture_output=False)

source_path = Path(source_path_str)

target_path = Path(target_path_str)

shutil.copy(source_path_str, target_path)
