#!/usr/bin/env python3
import os
import sys

def main():
    body = sys.stdin.buffer.read()
    sys.stdout.write("CGI ECHO\n")
    sys.stdout.write(f"PATH_INFO={os.environ.get('PATH_INFO','')}\n")
    if body:
        sys.stdout.buffer.write(body)

if __name__ == "__main__":
    main()
