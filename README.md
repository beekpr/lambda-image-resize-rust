# Lambda Image Resize

A simple image resize lambda function, written in Rust.

This binary responds to Amazon S3 events and triggers a resize on the uploaded image with the sizes specified. Right now you can only resize on the width of an image.
LACEMENTS`, an array of replacements for the path (`export REPLACEMENTS="original:resized"`)

## Compile

Use [Lambda-Rust docker image](https://hub.docker.com/r/softprops/lambda-rust/) to compile this binary. With Docker running run the following command to build a release.

```
 docker run --rm  -v ${PWD}:/code  -v ${HOME}/.cargo/registry:/root/.cargo/registry  -v ${HOME}/.cargo/git:/root/.cargo/git  softprops/lambda-rust

```
