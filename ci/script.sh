# This script takes care of testing your crate

set -ex

# TODO This is the "test phase", tweak it as you see fit
main() {
    cross build --target $TARGET
    cross build --target $TARGET --release

    if [ ! -z $DISABLE_TESTS ]; then
        return
    elif [ ! -z $SKIP_DOCTEST ]; then
      cross test --target $TARGET --lib
      cross test --target $TARGET --release --lib
    else
      cross test --target $TARGET
      cross test --target $TARGET --release
    fi
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
