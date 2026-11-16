//! X-04 operation table: the HTTP routes resolve to the operations and execution shapes the
//! router had before the table existed (no database needed).

use ieo_coord::api::{Operation, Shape, route};

#[test]
fn routes_resolve_to_their_operations_and_shapes() {
    let cases: Vec<(&str, &str, Option<Operation>)> = vec![
        ("GET", "/v0/health", Some(Operation::Health)),
        ("GET", "/v0/health/", Some(Operation::Health)),
        ("POST", "/v0/health", None),
        (
            "GET",
            "/v0/repos/r/roles",
            Some(Operation::RoleStatus { repo: "r" }),
        ),
        (
            "POST",
            "/v0/repos/r/stop",
            Some(Operation::RoleStop { repo: "r" }),
        ),
        ("POST", "/v0/sprints", Some(Operation::Register)),
        ("POST", "/v0/sprint-inputs", Some(Operation::Convert)),
        ("POST", "/v0/imports/notion", Some(Operation::ImportNotion)),
        (
            "POST",
            "/v0/coordinator/exceptions",
            Some(Operation::RecordException),
        ),
        (
            "GET",
            "/v0/sprints/s/revisions/1",
            Some(Operation::GetRevision { id: "s", rev: "1" }),
        ),
        (
            "POST",
            "/v0/sprints/s/revisions/1/semantic-review",
            Some(Operation::SemanticReview { id: "s", rev: "1" }),
        ),
        (
            "GET",
            "/v0/sprints/s/state",
            Some(Operation::SprintState { id: "s" }),
        ),
        (
            "GET",
            "/v0/sprints/s/events",
            Some(Operation::ListEvents { id: "s" }),
        ),
        ("POST", "/v0/agent/reports", Some(Operation::SubmitReport)),
        (
            "POST",
            "/v0/agent/repro-requests",
            Some(Operation::SubmitRepro),
        ),
        (
            "GET",
            "/v0/agent/repro-requests/abc",
            Some(Operation::GetRepro { id: "abc" }),
        ),
        // the trailing empty segment is trimmed: no request id, no operation (X-03 review P-1)
        ("GET", "/v0/agent/repro-requests/", None),
        (
            "POST",
            "/v0/coordinator/assignments",
            Some(Operation::Assign),
        ),
        (
            "POST",
            "/v0/repos/r/roles/development/checkpoints",
            Some(Operation::ReportCheckpoint {
                repo: "r",
                role: "development",
            }),
        ),
        // an inner empty segment is kept (an empty repo_id reaches the handler)
        (
            "POST",
            "/v0/repos//roles/guardrail/checkpoints",
            Some(Operation::ReportCheckpoint {
                repo: "",
                role: "guardrail",
            }),
        ),
        (
            "GET",
            "/v0/repos/r/roles/x/checkpoints/latest",
            Some(Operation::LatestCheckpoint {
                repo: "r",
                role: "x",
            }),
        ),
        (
            "GET",
            "/v0/repos/r/roles/x/checkpoints/compare",
            Some(Operation::CompareCheckpoint {
                repo: "r",
                role: "x",
            }),
        ),
        ("POST", "/v0/coordinator/evidence", Some(Operation::Ingest)),
        ("POST", "/v0/coordinator/judgments", Some(Operation::Judge)),
        ("GET", "/v0/coordinator/outbox", Some(Operation::ListOutbox)),
        (
            "POST",
            "/v0/coordinator/outbox/7/delivery",
            Some(Operation::Deliver { id: "7" }),
        ),
        (
            "POST",
            "/v0/repos/r/roles/development/deliveries",
            Some(Operation::DeliverAssignment {
                repo: "r",
                role: "development",
            }),
        ),
        ("GET", "/v0/repos/r/roles/development/deliveries", None),
        // getAssignmentChain (02-05 step 4)
        (
            "GET",
            "/v0/coordinator/assignments/A1/generations/0/chain",
            Some(Operation::AssignmentChain {
                assignment_id: "A1",
                generation: "0",
            }),
        ),
        (
            "POST",
            "/v0/coordinator/assignments/A1/generations/0/chain",
            None,
        ),
        ("GET", "/v0/coordinator/assignments/A1/generations/0", None),
        ("GET", "/v0/agent/reports", None),
        ("GET", "/v0/repos/a/b/roles/x/checkpoints/latest", None),
    ];
    for (m, p, want) in cases {
        assert_eq!(route(m, p), want, "{m} {p}");
    }
    let shape = |m, p| route(m, p).map(|o| o.shape());
    assert_eq!(shape("GET", "/v0/health"), Some(Shape::Unauthenticated));
    assert_eq!(shape("POST", "/v0/repos/r/stop"), Some(Shape::NoDatabase));
    assert_eq!(
        shape("GET", "/v0/repos/r/roles/x/checkpoints/compare"),
        Some(Shape::CallThenTransaction)
    );
    // deliverAssignment (02-05 step 3): two transactions around the supervisor call
    assert_eq!(
        shape("POST", "/v0/repos/r/roles/development/deliveries"),
        Some(Shape::Delivery)
    );
    assert_eq!(
        shape("GET", "/v0/coordinator/assignments/A1/generations/0/chain"),
        Some(Shape::Transaction)
    );
    // the five operations of the gRPC surface are one transaction each (contract §9)
    for (m, p) in [
        ("POST", "/v0/agent/reports"),
        ("POST", "/v0/agent/repro-requests"),
        ("GET", "/v0/agent/repro-requests/abc"),
        ("POST", "/v0/repos/r/roles/development/checkpoints"),
        ("GET", "/v0/repos/r/roles/development/checkpoints/latest"),
    ] {
        assert_eq!(shape(m, p), Some(Shape::Transaction), "{m} {p}");
    }
}
