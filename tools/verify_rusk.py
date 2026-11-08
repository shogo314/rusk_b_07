#!/usr/bin/env python3
"""Connect Rusk compilation and its dependencies to verification-helper."""
import argparse
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    compile_parser = commands.add_parser('compile')
    compile_parser.add_argument('source', type=Path)
    compile_parser.add_argument('directory', type=Path)
    deps_parser = commands.add_parser('dependencies')
    deps_parser.add_argument('source', type=Path)
    args = parser.parse_args()
    if args.command == 'dependencies':
        subprocess.run(['cargo', 'build', '--locked', '--release'], cwd=ROOT, check=True)
        dependencies = subprocess.check_output(
            [str(ROOT / 'target/release/rusk'), 'dependencies', str(args.source.resolve())],
            text=True,
        )
        paths = [Path(path) for path in dependencies.splitlines()]
        paths.append(Path(__file__).resolve())
        paths += [ROOT / name for name in (
            'Cargo.toml', 'Cargo.lock', 'build.rs', 'rusk.par',
            'requirements-verify.txt', '.verify-helper/config.toml',
        )]
        paths += sorted((ROOT / 'src').rglob('*.rs'))
        paths += sorted((ROOT / 'lib').rglob('*.rusk'))
        paths += sorted((ROOT / 'lib').rglob('*.hpp'))
        for path in sorted(set(paths)):
            print(path)
        return
    source = args.source.resolve()
    directory = args.directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    subprocess.run(['cargo', 'build', '--locked', '--release'], cwd=ROOT, check=True)
    cpp = directory / 'main.cpp'
    subprocess.run([str(ROOT / 'target/release/rusk'), 'compile', str(source), '-o', str(cpp)], check=True)
    subprocess.run(['g++', '-std=c++20', '-O2', '-Wall', '-Wextra', str(cpp), '-o', str(directory / 'main')], check=True)


if __name__ == '__main__':
    main()
