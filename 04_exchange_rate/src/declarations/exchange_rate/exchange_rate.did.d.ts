import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';

export interface RatesWithInterval {
  'interval' : bigint,
  'rates' : Array<[bigint, number]>,
}
export interface TimeRange { 'end' : bigint, 'start' : bigint }
export interface _SERVICE {
  'get_rates' : ActorMethod<[TimeRange], RatesWithInterval>,
  'get_rates2' : ActorMethod<[], string>,
}
