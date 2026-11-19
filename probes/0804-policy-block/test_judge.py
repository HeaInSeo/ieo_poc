import unittest

from judge import MARKER, connection, detection


class JudgeTests(unittest.TestCase):
    def setUp(self):
        self.actor = {"metadata": {"uid": "actor"},
                      "spec": {"securityContext": {"runAsUser": 10001}},
                      "status": {"podIP": "10.244.0.2"}}
        self.receiver = {"metadata": {"uid": "receiver"}, "status": {"podIP": "10.244.1.2"}}
        self.case = {"destination": "10.244.1.2", "source": "development", "allowed": True}
        self.attempt = {"phase": "attempt", "marker": MARKER, "action": "connect", "role": "development",
                        "destination": "10.244.1.2", "pod_uid": "actor", "uid": 10001}
        self.receipts = [{"event": "accepted", "peer": "10.244.0.2", "pod_uid": "receiver"},
                         {"event": "received", "peer": "10.244.0.2",
                          "pod_uid": "receiver", "request_id": MARKER}]

    def check(self, result, receipts, rc):
        return connection(self.case, self.actor, self.receiver,
                          [self.attempt, {**self.attempt, "phase": "result", **result}], receipts, rc)

    def test_open_requires_both_accept_and_marker(self):
        result = {"ok": True, "stage": "echo"}
        self.assertTrue(self.check(result, self.receipts, 0))
        self.assertFalse(self.check(result, self.receipts[1:], 0))
        self.assertFalse(self.check(result, [{**self.receipts[0], "pod_uid": "other"}, self.receipts[1]], 0))
        self.assertFalse(self.check(result, self.receipts, 1))

    def test_policy_requires_connect_failure_and_no_accept(self):
        self.case["allowed"] = False
        result = {"ok": False, "stage": "connect", "error": "TimeoutError"}
        self.assertTrue(self.check(result, [], 1))
        self.assertFalse(self.check(result, self.receipts[:1], 1))
        self.assertFalse(self.check({**result, "stage": "echo"}, [], 1))
        self.assertFalse(self.check(result, [], 0))

    def test_violation_never_allows_completion(self):
        self.assertEqual(detection(True, True, ["matched"], ["matched"]),
                         {"verdict": "VIOLATION", "completion_eligible": False})

    def test_missing_or_foreign_hook_is_inconclusive(self):
        for args in ((True, True, ["matched"], []), (True, True, ["matched"], ["foreign"]),
                     (False, True, ["matched"], ["matched"]), (True, False, ["matched"], ["matched"])):
            self.assertEqual(detection(*args), {"verdict": "INCONCLUSIVE", "completion_eligible": False})


if __name__ == "__main__":
    unittest.main(verbosity=2)
