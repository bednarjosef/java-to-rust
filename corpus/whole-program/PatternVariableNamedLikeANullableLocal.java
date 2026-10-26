import java.util.ArrayList;
import java.util.List;

// A pattern variable with the name of an earlier local that may be null, as Minecraft's
// `DefendVillageTargetGoal.canUse` has: a loop's `player` over a list that may hold null, then
// `target instanceof Player player`, which is never null.
public class PatternVariableNamedLikeANullableLocal {
    static class Entity {
        final String name;

        Entity(String name) {
            this.name = name;
        }
    }

    static class Player extends Entity {
        final boolean creative;

        Player(String name, boolean creative) {
            super(name);
            this.creative = creative;
        }

        boolean isCreative() {
            return creative;
        }
    }

    Entity potentialTarget;

    boolean canUse(List<Player> players) {
        for (Player player : players) {
            if (player == null || player.name.startsWith("v")) {
                this.potentialTarget = player;
            }
        }
        return this.potentialTarget == null ? false : !(this.potentialTarget instanceof Player player && player.isCreative());
    }

    public static void main(String[] args) {
        List<Player> players = new ArrayList<>();
        players.add(new Player("steve", false));
        players.add(new Player("vex", true));
        PatternVariableNamedLikeANullableLocal goal = new PatternVariableNamedLikeANullableLocal();
        System.out.println(goal.canUse(players));
        players.add(null);
        System.out.println(goal.canUse(players) + " " + goal.potentialTarget);
        goal.potentialTarget = new Entity("pig");
        System.out.println(goal.canUse(List.of(new Player("alex", false))));
    }
}
