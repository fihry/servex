#!/usr/bin/env python3
import os

keys = ["PATH_INFO", "REQUEST_METHOD", "CONTENT_TYPE"]
for k in keys:
    print(f"{k}={os.environ.get(k, '')}")
