import { describe, expect, it } from 'vitest';
import { isBooking, isCourt, isCourtPage, isSlotList, isGamePage, isGameDetail, isUser, isPersonalBookingPage, isPayment } from './validators';

const id = '11111111-1111-4111-8111-111111111111';
const start_time = '2026-10-05T10:00:00Z';
const end_time = '2026-10-05T11:00:00Z';
const page = {total: 1, page: 1, per_page: 12};
const court = {id, name: 'Sân', address: 'Địa chỉ', sport_type: 'pickleball', price_per_slot: 100001, photo_urls: [], lat: 10.5, lng: 106.5, distance_km: null};
const slot = {id, start_time, end_time, status: 'available'};
const booking = {slot_id: id, court_id: id, start_time, end_time, status: 'booked', booked_by: id};
const creator = {display_name: null, avatar_url: null};
const summary = {id, sport_type: 'pickleball', court: {id, name: 'Sân', address: 'Địa chỉ', distance_km: null}, slot: {start_time, end_time}, max_players: 4, current_players: 1, status: 'open', creator};
const split = {total_amount: 100001, per_player: 25000, remainder: 1, creator_pays_extra: true, player_count: 4, paid_total: 0, remaining_total: 100001, payments: [{user_id: id, display_name: null, amount: 25001, paid: false}]};
const game = {id, creator_id: id, court_slot_id: id, sport_type: 'pickleball', max_players: 4, status: 'open', description: null, court: {id, name: 'Sân', address: 'Địa chỉ', price_per_slot: 100001}, slot: {start_time, end_time}, players: [{user_id: id, ...creator, payment_status: null}], split};
const user = {id, email: null, phone: '0901234567', ...creator, created_at: start_time};
const payment = {id, amount: 25001, provider: 'momo', status: 'pending', pay_url: null, expires_at: end_time};
const personal = {...booking, court_name: 'Sân', sport_type: 'pickleball', game_id: null};
const fixtures: [string, (value: unknown) => boolean, unknown][] = [
  ['court', isCourt, court], ['court page', isCourtPage, {...page, items: [court]}], ['slots', isSlotList, {slots: [slot]}],
  ['booking', isBooking, booking], ['game page', isGamePage, {...page, games: [summary]}], ['game detail', isGameDetail, game],
  ['user', isUser, user], ['personal bookings', isPersonalBookingPage, {...page, bookings: [personal]}], ['payment', isPayment, payment],
];
describe('server response validators', () => {
  it.each(fixtures)('accepts a valid %s DTO including nullable fields and extra server metadata', (_label, validate, value) => {
    expect(validate(value)).toBe(true);
    expect(validate({...value as object, extra_server_metadata: true})).toBe(true);
  });
  it.each(fixtures)('rejects malformed top-level %s payloads', (_label, validate) => {
    for (const value of [null, {}, [], 'text', 0]) expect(validate(value)).toBe(false);
  });
  it('rejects malformed arrays and nested objects', () => {
    expect(isCourtPage({...page, items: [null]})).toBe(false);
    expect(isSlotList({slots: {}})).toBe(false);
    expect(isGamePage({...page, games: [{...summary, court: null}]})).toBe(false);
    expect(isGameDetail({...game, players: [null]})).toBe(false);
    expect(isGameDetail({...game, split: {...split, payments: [{user_id: id}]}})).toBe(false);
    expect(isPersonalBookingPage({...page, bookings: [{...personal, game_id: 'bad'}]})).toBe(false);
  });
  it('rejects unknown enum values', () => {
    expect(isCourt({...court, sport_type: 'tennis'})).toBe(false);
    expect(isSlotList({slots: [{...slot, status: 'free'}]})).toBe(false);
    expect(isGameDetail({...game, status: 'finished'})).toBe(false);
    expect(isGameDetail({...game, players: [{...game.players[0], payment_status: 'unknown'}]})).toBe(false);
    expect(isPayment({...payment, provider: 'cash'})).toBe(false);
    expect(isPayment({...payment, status: 'unknown'})).toBe(false);
  });
  it('rejects nonfinite, fractional, unsafe and incorrectly typed numeric fields', () => {
    for (const amount of [NaN, Infinity, 1.5, -1, Number.MAX_SAFE_INTEGER + 1, '100']) {
      expect(isCourt({...court, price_per_slot: amount})).toBe(false);
      expect(isPayment({...payment, amount})).toBe(false);
      expect(isGameDetail({...game, max_players: amount})).toBe(false);
    }
    expect(isCourt({...court, lat: NaN})).toBe(false);
    expect(isGamePage({...page, total: -1, games: []})).toBe(false);
    expect(isGamePage({...page, page: 0, games: []})).toBe(false);
  });
  it('rejects unusable IDs and dates and allows optional distance', () => {
    expect(isUser({...user, id: 'bad'})).toBe(false);
    expect(isUser({...user, created_at: 'not a date'})).toBe(false);
    expect(isPayment({...payment, expires_at: null})).toBe(true);
    expect(isGameDetail({...game, split: null})).toBe(true);
    expect(isCourt({...court, distance_km: undefined})).toBe(true);
  });
});
