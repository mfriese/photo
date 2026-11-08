import unittest
from check_architecture import violations


def packages(**dependencies):
    return {"packages": [{"name": name, "dependencies": [{"name": dep} for dep in deps]}
                         for name, deps in dependencies.items()]}


class ArchitectureTests(unittest.TestCase):
    def test_current_layer_direction_is_allowed(self):
        self.assertEqual(violations(packages(**{
            "photosuite-domain": ["chrono"], "photosuite-application": ["photosuite-domain"],
            "photosuite": ["eframe", "photosuite-domain", "photosuite-application"],
        })), [])

    def test_gui_or_adapter_dependency_in_either_core_layer_is_rejected(self):
        for layer in ["photosuite-domain", "photosuite-application"]:
            for forbidden in ["eframe", "image", "photosuite", "walkdir"]:
                deps = {"photosuite-domain": ["chrono"], "photosuite-application": ["photosuite-domain"]}
                deps[layer].append(forbidden)
                self.assertTrue(violations(packages(**deps)))

    def test_missing_core_package_is_not_silently_skipped(self):
        self.assertTrue(violations({"packages": []}))
