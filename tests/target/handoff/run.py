"""Run Hub's production Instance consumer in the pinned System handoff fixture."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

SYSTEM = 'f97524817ce240a0b0471be3b557ceb5a871fe14'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('kernel', type=Path)
    parser.add_argument('--system-checkout', type=Path)
    parser.add_argument('--profile', choices=('debug', 'release'), default='release')
    parser.add_argument('--harts', choices=(1, 2, 4), type=int, nargs='+', default=[1])
    args = parser.parse_args()
    test = Path(__file__).resolve().parent
    hub = test.parents[2]
    target = test / 'target'
    target.mkdir(parents=True, exist_ok=True)
    overlay = target / 'system'
    if not overlay.exists():
        source = str(args.system_checkout.resolve()) if args.system_checkout else 'https://github.com/shuyra-arche/system.git'
        subprocess.run(['git', 'clone', '--no-hardlinks', source, overlay], check=True)
        subprocess.run(['git', 'checkout', '--detach', SYSTEM], cwd=overlay, check=True)
    actual = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=overlay, text=True).strip()
    assert actual == SYSTEM, f'wrong System source: {actual}'
    # Build from immutable sources on every invocation, preserving prior result
    # directories under ignored target/. The original checkout is never edited.
    for path in ['tests/guest/handoff/Cargo.toml', 'tests/guest/handoff/main.rs', 'tests/guest/system/run.py']:
        original = subprocess.check_output(['git', 'show', f'{SYSTEM}:{path}'], cwd=overlay)
        (overlay / path).write_bytes(original)
    user = overlay / 'tests/guest/handoff'
    manifest = user / 'Cargo.toml'
    text = manifest.read_text()
    text = text.replace('system-protocol = { package = "system-api", path = "../../../api" }',
                        f'system-protocol = {{ package = "system-api", git = "https://github.com/shuyra-arche/system.git", rev = "{SYSTEM}" }}')
    text = text.replace('system-client = { path = "../../../client" }',
                        f'system-client = {{ git = "https://github.com/shuyra-arche/system.git", rev = "{SYSTEM}" }}')
    text = text.replace('[profile.release]', f'hub-api = {{ path = "{hub / "api"}" }}\nhub-server = {{ path = "{hub / "server"}" }}\n[profile.release]')
    manifest.write_text(text)
    main = user / 'main.rs'
    text = main.read_text()
    text = text.replace('fn controller(license: PendId, context: Context, role: usize) {',
                        'fn controller(license: PendId, context: Context, role: usize) {\n    if role == 0 { hub::controller(license, context); return; }')
    text = text.replace('use alloc::{string::String, vec};', f'#[path = "{test / "controller.rs"}"]\nmod hub;\nuse alloc::{{string::String, vec}};')
    # Tighten the retained provider Driver probe: only native denial proves
    # the withdrawn recipient token is unusable, not an unrelated failure.
    old = 'assert!(!error.source.is_busy(), "Busy is not revocation evidence");'
    assert text.count(old) == 1, 'provider withdrawal probe changed'
    text = text.replace(old, 'assert_eq!(error.source, kernel_api::abi::call::sign::SignFail::Denied);')
    main.write_text(text)
    runner = overlay / 'tests/guest/system/run.py'
    text = runner.read_text().replace("(('handoff', 'auth', 'cancel', 'death') if args.handoff else (0, 2, 1))", "(('handoff',) if args.handoff else (0, 2, 1))")
    runner.write_text(text)
    shutil.copyfile(test / 'Cargo.lock', user / 'Cargo.lock')
    inputs = {
        'system_revision': SYSTEM,
        'hub_revision': subprocess.check_output(['git', '-c', f'safe.directory={hub}', 'rev-parse', 'HEAD'], cwd=hub, text=True).strip(),
        'hub_dirty': bool(subprocess.check_output(['git', '-c', f'safe.directory={hub}', 'status', '--porcelain'], cwd=hub)),
        'controller_sha256': hashlib.sha256((test / 'controller.rs').read_bytes()).hexdigest(),
        'hub_lock_sha256': hashlib.sha256((hub / 'Cargo.lock').read_bytes()).hexdigest(),
        'note': 'CPU-only Machine/Signal/Instance consumer; no standalone Hub Unit or hardware authorization proof',
    }
    (target / 'inputs.json').write_text(json.dumps(inputs, indent=2) + '\n')
    command = ['python3', runner, args.kernel.resolve(), '--handoff', '--profile', args.profile,
               '--harts', *map(str, args.harts)]
    evidence = overlay / 'tests/guest/system/target/handoff' / args.profile
    previous_logs = set(evidence.rglob('users-handoff-harts-*.log')) if evidence.exists() else set()
    result = subprocess.run(command)
    # The provider runner retains every VM log, archive, ELF, lock and receipt.
    # Keep a stable pointer without copying or overwriting prior failures.
    (target / 'evidence.txt').write_text(str(overlay / 'tests/guest/system/target/handoff') + '\n')
    if result.returncode:
        raise SystemExit(result.returncode)
    logs = set(evidence.rglob('users-handoff-harts-*.log')) - previous_logs
    assert logs and all(
        b'PASS hub-consumer native=1' in path.read_bytes()
        and b'PASS hub-lease native=1 replay=1 cancel=1 ancestor-revoke=1 withdrawal=1' in path.read_bytes()
        and b'PASS hub-machine native-authority=1 prepared-target=1 slots=2 managed-revoke=1 retained-device=1' in path.read_bytes()
        for path in logs
    ), 'missing Hub consumer or native lease assertion'


if __name__ == '__main__':
    main()
