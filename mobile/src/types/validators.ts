import type { Court, Page, SlotList, GamePage, GameDetail, User, PersonalBookingPage, Payment, Split, GameSummary, Booking } from './api';

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const string = (value: unknown): value is string => typeof value === 'string';
const nullableString = (value: unknown) => value === null || string(value);
const uuid = (value: unknown) => string(value) && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
const date = (value: unknown) => string(value) && Number.isFinite(Date.parse(value));
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
const integer = (value: unknown) => finite(value) && Number.isSafeInteger(value);
const count = (value: unknown) => integer(value) && (value as number) >= 0;
const oneOf = (value: unknown, values: readonly string[]) => string(value) && values.includes(value);
const sport = (value: unknown) => oneOf(value, ['pickleball', 'mini_football']);
const gameStatus = (value: unknown) => oneOf(value, ['open', 'full', 'in_progress', 'completed', 'cancelled']);
const paymentStatus = (value: unknown) => oneOf(value, ['pending', 'paid', 'expired', 'refunded', 'disputed']);
const slotStatus = (value: unknown) => oneOf(value, ['available', 'booked', 'locked']);
const array = (value: unknown, check: (item: unknown) => boolean) => Array.isArray(value) && value.every(check);
const pagination = (value: Record<string, unknown>) => count(value.total) && integer(value.page) && (value.page as number) >= 1 && integer(value.per_page) && (value.per_page as number) >= 1;
const slotTimes = (value: unknown) => object(value) && date(value.start_time) && date(value.end_time);

export function isUser(value: unknown): value is User {
  return object(value) && uuid(value.id) && nullableString(value.email) && string(value.phone) && nullableString(value.display_name) && nullableString(value.avatar_url) && date(value.created_at);
}
export function isCourt(value: unknown): value is Court {
  return object(value) && uuid(value.id) && string(value.name) && string(value.address) && sport(value.sport_type) && count(value.price_per_slot) && array(value.photo_urls, string) && finite(value.lat) && finite(value.lng) && (value.distance_km === undefined || value.distance_km === null || finite(value.distance_km));
}
export function isCourtPage(value: unknown): value is Page<Court> {
  return object(value) && pagination(value) && array(value.items, isCourt);
}
export function isSlotList(value: unknown): value is SlotList {
  return object(value) && array(value.slots, (item) => object(item) && uuid(item.id) && slotTimes(item) && slotStatus(item.status));
}
export function isBooking(value: unknown): value is Booking {
  return object(value) && uuid(value.slot_id) && uuid(value.court_id) && slotTimes(value) && slotStatus(value.status) && (value.booked_by === null || uuid(value.booked_by));
}
function isGameSummary(value: unknown): value is GameSummary {
  return object(value) && uuid(value.id) && sport(value.sport_type) && gameStatus(value.status) && count(value.max_players) && count(value.current_players) &&
    object(value.court) && uuid(value.court.id) && string(value.court.name) && string(value.court.address) && (value.court.distance_km === null || finite(value.court.distance_km)) &&
    slotTimes(value.slot) && object(value.creator) && nullableString(value.creator.display_name) && nullableString(value.creator.avatar_url);
}
export function isGamePage(value: unknown): value is GamePage {
  return object(value) && pagination(value) && array(value.games, isGameSummary);
}
function isSplit(value: unknown): value is Split {
  return object(value) && count(value.total_amount) && count(value.per_player) && count(value.remainder) && typeof value.creator_pays_extra === 'boolean' && count(value.player_count) && count(value.paid_total) && integer(value.remaining_total) &&
    array(value.payments, (entry) => object(entry) && uuid(entry.user_id) && nullableString(entry.display_name) && count(entry.amount) && typeof entry.paid === 'boolean');
}
export function isGameDetail(value: unknown): value is GameDetail {
  return object(value) && uuid(value.id) && uuid(value.creator_id) && uuid(value.court_slot_id) && sport(value.sport_type) && gameStatus(value.status) && count(value.max_players) && nullableString(value.description) &&
    object(value.court) && uuid(value.court.id) && string(value.court.name) && string(value.court.address) && count(value.court.price_per_slot) && slotTimes(value.slot) &&
    array(value.players, (player) => object(player) && uuid(player.user_id) && nullableString(player.display_name) && nullableString(player.avatar_url) && (player.payment_status === null || paymentStatus(player.payment_status))) && (value.split === null || isSplit(value.split));
}
export function isPersonalBookingPage(value: unknown): value is PersonalBookingPage {
  return object(value) && pagination(value) && array(value.bookings, (entry) => isBooking(entry) && object(entry) && uuid(entry.booked_by) && string(entry.court_name) && sport(entry.sport_type) && (entry.game_id === null || uuid(entry.game_id)));
}
export function isPayment(value: unknown): value is Payment {
  return object(value) && uuid(value.id) && count(value.amount) && oneOf(value.provider, ['momo', 'zalopay']) && paymentStatus(value.status) && nullableString(value.pay_url) && (value.expires_at === null || date(value.expires_at));
}
