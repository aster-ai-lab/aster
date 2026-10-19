# Isolated AIX tree provider fixture

The fixture HTML and script are packaged into a real local AIX by native
capability tests. Renderer tests execute this same script in a separate JSDOM
window. The provider creates a MessageChannel, reads its document identifier
from the admitted URL fragment and transfers one endpoint to the production
view factory. The host sends its greeting only on that private endpoint.
They do not prove browser sandbox/CSP or hosted PG/RustFS acceptance.

The native ACL declaration is owned by the test package:

```acl
ui "tree" {
  title = "Tasks"
  entry = "ui/tree/index.html"
  root = "ui/tree"
  container = "resources"
  presentation = "tree"
}
```

The provider supplies roots, populates a child on expansion, responds to refresh,
and records selection and its own `open-task` action. It has no host credentials
or native service grants.
