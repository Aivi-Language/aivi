from pathlib import Path
import tempfile
import unittest

from version import release_version, validate_workspace


class VersionTests(unittest.TestCase):
    def test_stable_and_prerelease(self):
        for tag in ["v0.1.0", "v1.2.3", "v10.20.30"]:
            self.assertEqual(release_version(tag), (tag[1:], False))
        for tag in ["v1.0.0-rc.1", "v0.2.0-alpha", "v1.0.0-0", "v1.0.0-01a", "v1.0.0-beta-1"]:
            self.assertEqual(release_version(tag), (tag[1:], True))

    def test_reject_invalid_or_ambiguous_tags(self):
        for tag in ["1.2.3", "v1.2", "v01.2.3", "v1.02.3", "v1.2.03", "v1.2.3-01",
                    "v1.2.3-rc.01", "v1.2.3-", "v1.2.3-rc..1", "v1.2.3+build.1",
                    "v1.2.3\n", "v1.2.3-β", "v1.2.3;echo bad", "v1.2.3-rc/1"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release_version(tag)

    def test_workspace_and_lock_must_agree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "crate").mkdir()
            (root / "Cargo.toml").write_text('[workspace]\nmembers = ["crate"]\n[workspace.package]\nversion = "0.1.0"\n')
            (root / "crate/Cargo.toml").write_text('[package]\nname = "aivi-cli"\nversion.workspace = true\n')
            (root / "Cargo.lock").write_text('[[package]]\nname = "aivi-cli"\nversion = "0.1.0"\n')
            self.assertEqual(validate_workspace(root, "v0.1.0"), ("0.1.0", False))
            with self.assertRaisesRegex(ValueError, "does not match"):
                validate_workspace(root, "v0.2.0")
            (root / "Cargo.lock").write_text('[[package]]\nname = "aivi-cli"\nversion = "0.0.9"\n')
            with self.assertRaisesRegex(ValueError, "stale"):
                validate_workspace(root, "v0.1.0")
            (root / "crate/Cargo.toml").write_text('[package]\nname = "aivi-cli"\nversion = "0.1.0"\n')
            with self.assertRaisesRegex(ValueError, "inherit"):
                validate_workspace(root, "v0.1.0")


if __name__ == "__main__":
    unittest.main()
