/** Fixed-capacity numeric ring over a Float64Array: no allocation after construction. */
export class RingBuffer {
  private readonly data: Float64Array;
  private head = 0; // next write slot
  private count = 0;
  /** Bumped on every change, so the render loop can dirty-check. */
  version = 0;

  constructor(readonly capacity: number) {
    if (!Number.isInteger(capacity) || capacity <= 0) {
      throw new RangeError('RingBuffer capacity must be a positive integer');
    }
    this.data = new Float64Array(capacity);
  }

  get length(): number {
    return this.count;
  }

  push(value: number): void {
    this.data[this.head] = value;
    this.head = (this.head + 1) % this.capacity;
    if (this.count < this.capacity) this.count++;
    this.version++;
  }

  /** Update the newest sample without consuming history capacity. */
  replaceLast(value: number): void {
    if (!this.count) { this.push(value); return; }
    this.data[(this.head - 1 + this.capacity) % this.capacity] = value;
    this.version++;
  }

  /** Sample `i`, 0 = oldest. NaN when out of range. */
  at(i: number): number {
    if (!(i >= 0 && i < this.count)) return NaN;
    return this.data[(this.head - this.count + i + this.capacity) % this.capacity]!;
  }

  last(): number {
    return this.at(this.count - 1);
  }

  clear(): void {
    this.head = 0;
    this.count = 0;
    this.version++;
  }

  toArray(): number[] {
    const out: number[] = [];
    for (let i = 0; i < this.count; i++) out.push(this.at(i));
    return out;
  }
}
