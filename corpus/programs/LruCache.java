// A least-recently-used cache: a hash table of chained entries (an array of
// buckets) and a doubly linked list that keeps the most recent entry in front.
public class LruCache {
    static class Entry {
        int key;
        int value;
        Entry hashNext;
        Entry prev;
        Entry next;

        Entry(int key, int value) {
            this.key = key;
            this.value = value;
        }
    }

    static class Cache {
        final int capacity;
        final Entry[] buckets;
        Entry head;
        Entry tail;
        int size;
        int hits;
        int misses;
        int evictions;

        Cache(int capacity) {
            this.capacity = capacity;
            this.buckets = new Entry[capacity * 2];
        }

        void unlinkList(Entry e) {
            if (e.prev != null) e.prev.next = e.next; else head = e.next;
            if (e.next != null) e.next.prev = e.prev; else tail = e.prev;
            e.prev = null;
            e.next = null;
        }

        void pushFront(Entry e) {
            e.next = head;
            if (head != null) head.prev = e;
            head = e;
            if (tail == null) tail = e;
        }

        void touch(Entry e) {
            if (head == e) return;
            unlinkList(e);
            pushFront(e);
        }
    }

    // Walks a bucket's chain; recursion keeps the cursor out of a loop.
    static Entry findIn(Entry e, int key) {
        if (e == null) return null;
        if (e.key == key) return e;
        return findIn(e.hashNext, key);
    }

    // Returns the chain without `target`.
    static Entry without(Entry chain, Entry target) {
        if (chain == null) return null;
        if (chain == target) return chain.hashNext;
        chain.hashNext = without(chain.hashNext, target);
        return chain;
    }

    static Entry find(Cache c, int key) {
        return findIn(c.buckets[key % c.buckets.length], key);
    }

    static int get(Cache c, int key) {
        Entry e = find(c, key);
        if (e == null) {
            c.misses++;
            return -1;
        }
        c.hits++;
        c.touch(e);
        return e.value;
    }

    static void put(Cache c, int key, int value) {
        Entry existing = find(c, key);
        if (existing != null) {
            existing.value = value;
            c.touch(existing);
            return;
        }
        if (c.size == c.capacity) {
            Entry victim = c.tail;
            c.unlinkList(victim);
            int idx = victim.key % c.buckets.length;
            c.buckets[idx] = without(c.buckets[idx], victim);
            c.size--;
            c.evictions++;
        }
        Entry fresh = new Entry(key, value);
        int slot = key % c.buckets.length;
        fresh.hashNext = c.buckets[slot];
        c.buckets[slot] = fresh;
        c.pushFront(fresh);
        c.size++;
    }

    static void showFrom(Entry e) {
        if (e == null) return;
        System.out.print(" " + e.key + ":" + e.value);
        showFrom(e.next);
    }

    public static void main(String[] args) {
        Cache c = new Cache(3);
        put(c, 1, 100);
        put(c, 2, 200);
        put(c, 3, 300);
        System.out.println(get(c, 1));
        put(c, 4, 400);
        System.out.println(get(c, 2));
        put(c, 3, 333);
        System.out.println(get(c, 3));
        put(c, 5, 500);
        System.out.print("recent:");
        showFrom(c.head);
        System.out.println();
        System.out.println(get(c, 1) + " " + get(c, 4) + " " + get(c, 5));
        put(c, 7, 700);
        put(c, 1, 111);
        System.out.print("recent:");
        showFrom(c.head);
        System.out.println();
        System.out.println("size " + c.size + " evictions " + c.evictions);

        Cache big = new Cache(8);
        int checksum = 0;
        for (int i = 0; i < 60; i++) {
            int key = (i * 7) % 13;
            if (i % 3 == 0) {
                put(big, key, i);
            } else {
                checksum += get(big, key);
            }
        }
        System.out.println("checksum " + checksum);
        System.out.println("hits " + big.hits + " misses " + big.misses + " evictions " + big.evictions);
        System.out.print("final:");
        showFrom(big.head);
        System.out.println();
        Cache empty = new Cache(1);
        System.out.println(get(empty, 9) + " " + (empty.head == null) + " " + (empty.tail == null));
    }
}
