export type Sport = 'pickleball' | 'mini_football';
export type GameStatus = 'open' | 'full' | 'in_progress' | 'completed' | 'cancelled';
export type PaymentStatus = 'pending' | 'paid' | 'expired' | 'refunded' | 'disputed';
export interface User { id: string; email: string | null; phone: string; display_name: string | null; avatar_url: string | null; created_at: string }
export interface Session { access_token: string; refresh_token: string; user: User }
export interface Court { id: string; name: string; address: string; sport_type: Sport; price_per_slot: number; photo_urls: string[]; lat: number; lng: number; distance_km?: number | null }
export interface Page<T> { items: T[]; total: number; page: number; per_page: number }
export interface Slot { id: string; start_time: string; end_time: string; status: 'available' | 'booked' | 'locked' }
export interface SlotList { slots: Slot[] }
export interface GamePage { games: GameSummary[]; total: number; page: number; per_page: number }
export interface Booking extends Omit<Slot, 'id'> { slot_id: string; court_id: string; booked_by: string | null }
export interface PersonalBooking extends Booking { court_name: string; sport_type: Sport; booked_by: string; game_id: string | null }
export interface PersonalBookingPage { bookings: PersonalBooking[]; total: number; page: number; per_page: number }
export interface GameSummary { id: string; sport_type: Sport; court: {id: string; name: string; address: string; distance_km: number | null}; slot: {start_time: string; end_time: string}; max_players: number; current_players: number; status: GameStatus; creator: {display_name: string | null; avatar_url: string | null} }
export interface Split { total_amount: number; per_player: number; remainder: number; creator_pays_extra: boolean; player_count: number; paid_total: number; remaining_total: number; payments: {user_id: string; display_name: string | null; amount: number; paid: boolean}[] }
export interface GameDetail { id: string; creator_id: string; court_slot_id: string; sport_type: Sport; max_players: number; status: GameStatus; description: string | null; court: {id: string; name: string; address: string; price_per_slot: number}; slot: {start_time: string; end_time: string}; players: {user_id: string; display_name: string | null; avatar_url: string | null; payment_status: PaymentStatus | null}[]; split: Split | null }
export interface Payment { id: string; amount: number; provider: 'momo' | 'zalopay'; status: PaymentStatus; pay_url: string | null; expires_at: string | null }
