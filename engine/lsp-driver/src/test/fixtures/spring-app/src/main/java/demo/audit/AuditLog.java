package demo.audit;

/** No class implements this: injecting it fails. */
public interface AuditLog {
    void record(String event);
}
