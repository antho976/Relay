import java.util.*;
import java.time.*;
import com.forge.app.domain.adapt.*;
import com.forge.app.domain.coach.*;
import com.forge.app.data.db.entities.*;
import com.forge.app.data.db.types.*;
import com.forge.app.program.*;
public class AvexDomainProbe {
 static LoggedSet set(double w,int reps,Integer duration) {return new LoggedSet(1,1,0,""+w,w,reps,1,null,false,false,false,null,null,null,duration);}
 public static void main(String[] args) {
  var low = new Recommendation.ReadinessScale(-5,"low",Confidence.MEDIUM,null);
  for (var phase : new BlockPhase[]{null,BlockPhase.DELOAD}) {
   var result=ProgressionAdvisor.INSTANCE.suggestNextLoad("bench","Bench",List.of(set(100,8,null)),EffortRating.JUST_RIGHT,"8-12",ExerciseUnit.WEIGHT,15,IntensityIntent.NORMAL,low,null,false,false,phase,new AdaptThresholds());
   System.out.println("below-top readiness=-5 phase="+phase+" target="+result.getTargetWeightLb()+" reason="+result.getReason());
  }
  var slot=new ProgramSlotSnap("bench","Bench",MuscleGroup.CHEST,ExerciseUnit.WEIGHT,List.of(),3,"8-12",List.of());
  List<ExerciseBout> bouts=new ArrayList<>();
  for(int i=0;i<5;i++) bouts.add(new ExerciseBout(i*86400000L,EffortRating.JUST_RIGHT,true,false,null,List.of(set(45,0,60)),"bench","normal"));
  var snap=new AdaptationSnapshot(20*86400000L,ZoneOffset.UTC,List.of(new ProgramDaySnap("day","Day",List.of(slot))),List.of(),Map.of("bench",bouts),List.of(),List.of(),List.of(),new PrefsSnap(),new HealthSnap());
  try {System.out.println("weighted hold evaluation="+ProgressionAdvisor.INSTANCE.evaluate(snap,new AdaptThresholds(),null));}
  catch(Exception e) {System.out.println("weighted hold evaluation threw "+e);e.printStackTrace(System.out);}
  var decision=new CoachDecision(1,"week","swap","bench","Bench","swap","reason","applied","day","db-row",1L,"pending",null,null,"week","week",null);
  for (var testedBouts : List.of(List.of(new ExerciseBout(86400000L,null,false,false,null,List.<LoggedSet>of(),"db-row","normal")), List.of(new ExerciseBout(86400000L,null,false,false,null,List.of(set(50,10,null)),"barbell-row","normal")))) {
   var outcomeSnap=new AdaptationSnapshot(20*86400000L,ZoneOffset.UTC,List.of(),List.of(),Map.of("bench",testedBouts),List.of(),List.of(),List.of(),new PrefsSnap(),new HealthSnap());
   System.out.println("swap sets="+testedBouts.get(0).getSets().size()+" performed="+testedBouts.get(0).getPerformedExerciseId()+" requested="+decision.getPayload()+" outcome="+OutcomeWatcher.INSTANCE.evaluate(List.of(decision),outcomeSnap,new AdaptThresholds(),LifeEvents.State.Companion.getNONE()));
  }
  System.out.println("not-followed outcome="+CoachOutcome.INSTANCE.label("applied","not_followed",1L,20*86400000L,ZoneOffset.UTC));
 }
}
