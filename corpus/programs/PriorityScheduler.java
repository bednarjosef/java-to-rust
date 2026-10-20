public class PriorityScheduler {
    /** A binary min-heap of ints kept in an array that doubles when it is full. */
    static class MinHeap {
        private int[] data = new int[4];
        private int size;

        int size() {
            return size;
        }

        boolean isEmpty() {
            return size == 0;
        }

        void push(int value) {
            if (size == data.length) {
                int[] bigger = new int[data.length * 2];
                for (int i = 0; i < size; i++) {
                    bigger[i] = data[i];
                }
                data = bigger;
            }
            int i = size;
            size++;
            data[i] = value;
            while (i > 0 && data[(i - 1) / 2] > data[i]) {
                swap(i, (i - 1) / 2);
                i = (i - 1) / 2;
            }
        }

        /** The smallest value; the caller checks isEmpty first. */
        int peek() {
            return data[0];
        }

        int pop() {
            int top = data[0];
            size--;
            data[0] = data[size];
            int i = 0;
            while (true) {
                int left = 2 * i + 1;
                int right = left + 1;
                int smallest = i;
                if (left < size && data[left] < data[smallest]) {
                    smallest = left;
                }
                if (right < size && data[right] < data[smallest]) {
                    smallest = right;
                }
                if (smallest == i) {
                    break;
                }
                swap(i, smallest);
                i = smallest;
            }
            return top;
        }

        private void swap(int a, int b) {
            int t = data[a];
            data[a] = data[b];
            data[b] = t;
        }
    }

    static int seed = 12345;

    static int nextInt(int bound) {
        seed = seed * 1664525 + 1013904223;
        return ((seed >>> 8) & 0xffff) % bound;
    }

    /** Heapsort through the heap: push everything, pop in order. */
    static int[] heapSort(int[] values) {
        MinHeap heap = new MinHeap();
        for (int v : values) {
            heap.push(v);
        }
        int[] out = new int[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = heap.pop();
        }
        return out;
    }

    /**
     * Huffman code lengths. A heap holds weight * 1024 + node, so the lightest subtree
     * comes out first; node ids below 512 are leaves, the rest are internal nodes.
     */
    static int[] huffmanDepths(int[] weights) {
        int n = weights.length;
        int[] weight = new int[2 * n];
        int[] left = new int[2 * n];
        int[] right = new int[2 * n];
        MinHeap heap = new MinHeap();
        for (int i = 0; i < n; i++) {
            weight[i] = weights[i];
            left[i] = -1;
            right[i] = -1;
            heap.push(weights[i] * 1024 + i);
        }
        int nodes = n;
        while (heap.size() > 1) {
            int a = heap.pop() % 1024;
            int b = heap.pop() % 1024;
            weight[nodes] = weight[a] + weight[b];
            left[nodes] = a;
            right[nodes] = b;
            heap.push(weight[nodes] * 1024 + nodes);
            nodes++;
        }
        int root = nodes - 1;
        int[] depth = new int[2 * n];
        int[] stack = new int[2 * n];
        int top = 0;
        stack[top++] = root;
        while (top > 0) {
            int node = stack[--top];
            if (left[node] >= 0) {
                depth[left[node]] = depth[node] + 1;
                depth[right[node]] = depth[node] + 1;
                stack[top++] = left[node];
                stack[top++] = right[node];
            }
        }
        int[] leafDepths = new int[n];
        for (int i = 0; i < n; i++) {
            leafDepths[i] = depth[i];
        }
        return leafDepths;
    }

    /** An event queue: each entry is time * 100 + task id, so ties go to the lower id. */
    static void simulateTasks(int taskCount, int horizon) {
        MinHeap events = new MinHeap();
        for (int id = 0; id < taskCount; id++) {
            events.push((nextInt(20) * 100) + id);
        }
        int processed = 0;
        int lastTime = -1;
        int maxBacklog = events.size();
        int checksum = 0;
        while (!events.isEmpty()) {
            int event = events.pop();
            int time = event / 100;
            int id = event % 100;
            if (time > horizon) {
                continue;
            }
            processed++;
            lastTime = time;
            checksum = (checksum * 31 + time * 7 + id) % 1000003;
            if (id % 3 == 0 && time < horizon) {
                events.push((time + 1 + nextInt(10)) * 100 + id);
            }
            maxBacklog = Math.max(maxBacklog, events.size());
        }
        System.out.println("tasks " + taskCount + ": processed " + processed + ", last time " + lastTime
                + ", max backlog " + maxBacklog + ", checksum " + checksum);
    }

    public static void main(String[] args) {
        MinHeap heap = new MinHeap();
        int[] pushed = {42, 7, 19, 3, 88, 3, 56, 21, 0, 15};
        for (int v : pushed) {
            heap.push(v);
        }
        System.out.print("pop order:");
        while (!heap.isEmpty()) {
            System.out.print(" " + heap.pop());
        }
        System.out.println();

        int[] values = new int[300];
        for (int i = 0; i < values.length; i++) {
            values[i] = nextInt(100000) - 50000;
        }
        int[] sorted = heapSort(values);
        boolean ordered = true;
        for (int i = 1; i < sorted.length; i++) {
            if (sorted[i - 1] > sorted[i]) {
                ordered = false;
            }
        }
        System.out.println("heap sort of 300 values ordered: " + ordered + ", min " + sorted[0] + ", max " + sorted[299]);
        System.out.println("median " + sorted[150] + ", sum " + sumOf(sorted));

        int[] frequencies = {5, 9, 12, 13, 16, 45};
        int[] depths = huffmanDepths(frequencies);
        long weighted = 0;
        for (int i = 0; i < frequencies.length; i++) {
            weighted += (long) frequencies[i] * depths[i];
            System.out.println("symbol " + i + " weight " + frequencies[i] + " depth " + depths[i]);
        }
        System.out.println("total encoded length " + weighted);

        simulateTasks(12, 60);
        simulateTasks(40, 200);

        MinHeap empty = new MinHeap();
        System.out.println("empty heap size " + empty.size() + ", pushed one: " + peekAfterPush(empty, 5));
    }

    static long sumOf(int[] values) {
        long total = 0;
        for (int v : values) {
            total += v;
        }
        return total;
    }

    static int peekAfterPush(MinHeap heap, int value) {
        heap.push(value);
        return heap.peek();
    }
}
