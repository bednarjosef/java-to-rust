// Features: a simple event system with a generic EventBus<E> holding Map<String, List<Listener<E>>>
// keyed by topic, a Listener functional interface, a listener interface with default methods,
// a named listener class and anonymous and lambda listeners, a sealed-free event hierarchy of
// abstract classes with subclasses, subscription handles with unsubscribe through an inner class
// that captures the bus, a dispatch loop that catches exceptions thrown by one listener and keeps
// going, and an ordered queue of pending events in an ArrayDeque.
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

public class EventListeners {
    static abstract class Event {
        private static int nextId = 1;
        final int id;

        Event() {
            this.id = nextId++;
        }

        abstract String topic();

        @Override
        public String toString() {
            return "#" + id + " " + topic();
        }
    }

    static class OrderPlaced extends Event {
        final String customer;
        final int total;

        OrderPlaced(String customer, int total) {
            this.customer = customer;
            this.total = total;
        }

        @Override
        String topic() {
            return "orders";
        }

        @Override
        public String toString() {
            return super.toString() + " for " + customer + " total " + total;
        }
    }

    static class PaymentFailed extends Event {
        final String reason;

        PaymentFailed(String reason) {
            this.reason = reason;
        }

        @Override
        String topic() {
            return "payments";
        }
    }

    interface Listener<E> {
        void onEvent(E event);

        default Listener<E> logging(String tag, List<String> sink) {
            return event -> {
                sink.add(tag + " saw " + event);
                onEvent(event);
            };
        }
    }

    static class Subscription {
        private final EventBus<?> bus;
        private final String topic;
        private final Listener<? extends Event> listener;

        Subscription(EventBus<?> bus, String topic, Listener<? extends Event> listener) {
            this.bus = bus;
            this.topic = topic;
            this.listener = listener;
        }

        void cancel() {
            bus.remove(topic, listener);
        }
    }

    static class EventBus<E extends Event> {
        private final Map<String, List<Listener<E>>> listeners = new TreeMap<>();
        private final Deque<E> pending = new ArrayDeque<>();
        final List<String> errors = new ArrayList<>();

        Subscription subscribe(String topic, Listener<E> listener) {
            listeners.computeIfAbsent(topic, t -> new ArrayList<>()).add(listener);
            return new Subscription(this, topic, listener);
        }

        void remove(String topic, Listener<? extends Event> listener) {
            List<Listener<E>> list = listeners.get(topic);
            if (list != null) {
                list.remove(listener);
            }
        }

        void post(E event) {
            pending.addLast(event);
        }

        int dispatchAll() {
            int delivered = 0;
            while (!pending.isEmpty()) {
                E event = pending.pollFirst();
                List<Listener<E>> list = listeners.getOrDefault(event.topic(), new ArrayList<>());
                for (Listener<E> listener : new ArrayList<>(list)) {
                    try {
                        listener.onEvent(event);
                        delivered++;
                    } catch (RuntimeException e) {
                        errors.add(event.id + ": " + e.getMessage());
                    }
                }
            }
            return delivered;
        }
    }

    static class Counter implements Listener<Event> {
        int count;

        @Override
        public void onEvent(Event event) {
            count++;
        }
    }

    public static void main(String[] args) {
        EventBus<Event> bus = new EventBus<>();
        List<String> log = new ArrayList<>();
        Counter counter = new Counter();
        bus.subscribe("orders", counter);
        bus.subscribe("orders", e -> log.add("lambda " + e));

        Listener<Event> anonymous = new Listener<Event>() {
            @Override
            public void onEvent(Event event) {
                if (event instanceof OrderPlaced order && order.total > 100) {
                    log.add("big order from " + order.customer);
                }
            }
        };
        bus.subscribe("orders", anonymous.logging("audit", log));

        Subscription failing = bus.subscribe("payments", e -> {
            PaymentFailed p = (PaymentFailed) e;
            if (p.reason.equals("fraud")) {
                throw new IllegalStateException("blocked: " + p.reason);
            }
            log.add("payment problem: " + p.reason);
        });

        bus.post(new OrderPlaced("ada", 50));
        bus.post(new OrderPlaced("grace", 250));
        bus.post(new PaymentFailed("card expired"));
        bus.post(new PaymentFailed("fraud"));
        int delivered = bus.dispatchAll();
        System.out.println("delivered " + delivered + " events, counter " + counter.count);
        for (String line : log) {
            System.out.println("  " + line);
        }
        for (String error : bus.errors) {
            System.out.println("error " + error);
        }

        failing.cancel();
        bus.post(new PaymentFailed("fraud"));
        System.out.println("after cancel delivered " + bus.dispatchAll() + ", errors " + bus.errors.size());

        Event last = new OrderPlaced("linus", 7);
        System.out.println("last event: " + last + " topic " + last.topic());
    }
}
