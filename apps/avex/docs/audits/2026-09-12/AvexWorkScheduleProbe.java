import androidx.work.impl.model.WorkSpec;
import androidx.work.BackoffPolicy;
import java.time.*;
public class AvexWorkScheduleProbe {
  public static void main(String[] args) throws Exception {
    var zone = ZoneId.of("America/Toronto");
    var launched = ZonedDateTime.of(2026,9,12,12,0,0,0,zone);
    var monday = ZonedDateTime.of(2026,9,14,12,0,0,0,zone);
    long delay = Duration.between(launched,monday).toMillis();
    var ctor = WorkSpec.Companion.class.getDeclaredConstructor(); ctor.setAccessible(true);
    long next = ctor.newInstance().calculateNextRunTime(false,0,BackoffPolicy.EXPONENTIAL,1800000L,launched.toInstant().toEpochMilli(),0,true,delay,21600000L,604800000L,Long.MAX_VALUE);
    System.out.println("Enqueued: "+launched+"; configured initial-delay target: "+monday);
    System.out.println("WorkManager 2.10.1 calculated first eligible run: "+Instant.ofEpochMilli(next).atZone(zone));
    long updatedDelay = Duration.between(launched.plusDays(1),monday).toMillis();
    long updated = ctor.newInstance().calculateNextRunTime(false,0,BackoffPolicy.EXPONENTIAL,1800000L,launched.toInstant().toEpochMilli(),0,true,updatedDelay,21600000L,604800000L,Long.MAX_VALUE);
    System.out.println("Reopening Sunday, UPDATE preserving original enqueue time: "+Instant.ofEpochMilli(updated).atZone(zone));
  }
}
