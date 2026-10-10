# Thin wrapper: every recipe runs ./dev, the single entry point (./dev help lists everything).
set shell := ["zsh", "-cu"]
set positional-arguments

default:
    @./dev help

help:
    @./dev help

setup:
    @./dev setup

check *args:
    @./dev check "$@"

build *args:
    @./dev build "$@"

run *args:
    @./dev run "$@"

demo:
    @./dev demo

gallery:
    @./dev gallery

smoke:
    @./dev smoke

test *args:
    @./dev test "$@"

lint:
    @./dev lint

fmt:
    @./dev fmt

bench:
    @./dev bench

visual:
    @./dev visual

visual-app:
    @./dev visual-app

visual-check:
    @./dev visual-check

visual-approve:
    @./dev visual-approve

capture *args:
    @./dev capture "$@"

genlog *args:
    @./dev genlog "$@"

bundle *args:
    @./dev bundle "$@"

live *args:
    @./dev live "$@"

pipe *args:
    @./dev pipe "$@"

clean *args:
    @./dev clean "$@"

linux-image:
    @./dev linux-image

linux-build:
    @./dev linux-build

linux-test:
    @./dev linux-test

linux-shot *args:
    @./dev linux-shot {{args}}

linux-shell:
    @./dev linux-shell

windows-build:
    @./dev windows-build

check-all:
    @./dev check-all

ci-test:
    @./dev ci-test
