if [ -z "$1" ]
  then
    echo "Requires a version argument, eg update-version.sh 0.6.0"
    exit 1
fi

find . -name "Cargo.toml" -exec sed -i "s/^version =.*/version = \"$1\"/" {} \;
cargo update -w
sed -i "s/version: .*/version: '$1',/" meson.build
