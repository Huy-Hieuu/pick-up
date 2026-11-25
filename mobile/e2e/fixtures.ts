import type { Page } from '@playwright/test';
import type { Booking, Court, GameDetail, GameStatus, GameSummary, Session, Slot } from '../src/types/api';
import { dateKey } from '../src/utils/format';

export const ids = { court: '11111111-1111-4111-8111-111111111111', slot: '22222222-2222-4222-8222-222222222222', game: '33333333-3333-4333-8333-333333333333', user: '44444444-4444-4444-8444-444444444444', creator: '55555555-5555-4555-8555-555555555555' };
export const session: Session = { access_token: 'fixture-access', refresh_token: 'fixture-refresh', user: { id: ids.user, email: 'player@example.test', phone: '0901234567', display_name: 'Minh An', avatar_url: null, created_at: '2026-01-01T00:00:00Z' } };
export const court: Court = { id: ids.court, name: 'Sân cộng đồng Thảo Điền', address: '12 Nguyễn Văn Hưởng, Thảo Điền, TP. Hồ Chí Minh', sport_type: 'pickleball', price_per_slot: 300000, photo_urls: [], lat: 10.8, lng: 106.7, distance_km: 1.2 };
export const slot: Slot = { id: ids.slot, start_time: dateKey(1) + 'T10:00:00Z', end_time: dateKey(1) + 'T11:00:00Z', status: 'available' };
export const paymentExpiresAt = new Date(Date.now() + 30 * 60000).toISOString();
export const summary: GameSummary = { id: ids.game, sport_type: 'pickleball', court: { id: ids.court, name: court.name, address: court.address, distance_km: 1.2 }, slot, max_players: 4, current_players: 1, status: 'open', creator: { display_name: 'Lan Anh', avatar_url: null } };

export async function mockApi(page: Page, mode: 'normal' | 'empty' | 'error' = 'normal', baseUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080') {
  const calls: { method: string; path: string; body: unknown }[] = [];
  let joined = false;
  let created = false;
  let paid = false;
  let booked = false;
  let gameStatus: GameStatus = 'open';
  const booking: Booking = { slot_id: ids.slot, court_id: ids.court, booked_by: ids.user, start_time: slot.start_time, end_time: slot.end_time, status: 'booked' };
  const detail = (): GameDetail => ({ id: ids.game, creator_id: created ? ids.user : ids.creator, court_slot_id: ids.slot, sport_type: 'pickleball', max_players: 4, status: gameStatus, description: 'Chơi vui, tôn trọng đồng đội.', court: { id: ids.court, name: court.name, address: court.address, price_per_slot: court.price_per_slot }, slot, players: [{ user_id: created ? ids.user : ids.creator, display_name: created ? 'Minh An' : 'Lan Anh', avatar_url: null, payment_status: paid && created ? 'paid' : 'pending' }, ...(joined && !created ? [{ user_id: ids.user, display_name: 'Minh An', avatar_url: null, payment_status: paid ? 'paid' as const : 'pending' as const }] : [])], split: { total_amount: 300000, per_player: joined && !created ? 150000 : 300000, remainder: 0, creator_pays_extra: false, player_count: joined && !created ? 2 : 1, paid_total: paid ? (created ? 300000 : 150000) : 0, remaining_total: paid ? (created ? 0 : 150000) : 300000, payments: [{ user_id: created ? ids.user : ids.creator, display_name: created ? 'Minh An' : 'Lan Anh', amount: joined && !created ? 150000 : 300000, paid: paid && created }, ...(joined && !created ? [{ user_id: ids.user, display_name: 'Minh An', amount: 150000, paid }] : [])] } });
  await page.route(baseUrl + '/**', async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname.replace(/\/$/, '');
    const method = request.method();
    const headers = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*', 'access-control-allow-methods': 'GET,POST,PATCH,OPTIONS' };
    if (method === 'OPTIONS') return route.fulfill({ status: 204, headers });
    const body = request.postData() ? request.postDataJSON() : undefined;
    calls.push({ method, path, body });
    const reply = (data: unknown, status = 200) => route.fulfill({ status, headers, contentType: 'application/json', body: JSON.stringify(data) });
    if (path === '/auth/login' || path === '/auth/refresh' || path === '/auth/verify-registration') return reply(session);
    if (path === '/auth/register') return reply({ email: body.email, expires_in: 300, message: 'Mã xác nhận đã được gửi.' }, 202);
    if (path === '/auth/resend-registration-otp') return reply({ message: 'Mã xác nhận đã được gửi.' });
    if (path === '/auth/logout') return route.fulfill({ status: 204, headers });
    if (path === '/auth/me') return reply(session.user);
    if (path === '/auth/me/bookings') return reply({ bookings: booked ? [{ ...booking, court_name: court.name, sport_type: court.sport_type, game_id: created ? ids.game : null }] : [], total: booked ? 1 : 0, page: 1, per_page: 20 });
    if (path === '/auth/me/games') return reply({ games: joined || created ? [summary] : [], total: joined || created ? 1 : 0, page: 1, per_page: 20 });
    if (path === '/courts' || path === '/games' && method === 'GET') {
      if (mode === 'error') return reply({ error: { code: 'SERVICE_UNAVAILABLE', message: 'Fixture outage' } }, 503);
      const items = mode === 'empty' ? [] : [path === '/courts' ? court : summary];
      return reply({ [path === '/courts' ? 'items' : 'games']: items, total: items.length, page: 1, per_page: 20 });
    }
    if (path === `/courts/${ids.court}`) return reply(court);
    if (path === `/courts/${ids.court}/slots`) return reply({ slots: [{ ...slot, status: booked ? 'booked' : 'available' }] });
    if (path === `/courts/${ids.court}/slots/${ids.slot}/book`) { booked = true; return reply(booking, 201); }
    if (path === `/courts/${ids.court}/slots/${ids.slot}/cancel`) { booked = false; return route.fulfill({ status: 204, headers }); }
    if (path === '/games' && method === 'POST') { created = true; return reply(detail(), 201); }
    if (path === `/games/${ids.game}/join`) { joined = true; return reply(detail()); }
    if (path === `/games/${ids.game}/leave`) { joined = false; return route.fulfill({ status: 204, headers }); }
    if (path === `/games/${ids.game}/status`) { gameStatus = body.status; return reply(detail()); }
    if (path === `/games/${ids.game}/cancel`) { gameStatus = 'cancelled'; return reply(detail()); }
    if (path === `/games/${ids.game}/split`) return reply(detail().split);
    if (path === `/games/${ids.game}/share`) return reply({ url: 'https://pickup.example.test/games/' + ids.game, message: 'Cùng tham gia trận PickUp!' });
    if (path === `/games/${ids.game}`) return reply(detail());
    if (path === `/games/${ids.game}/payments/initiate`) return reply({ id: '66666666-6666-4666-8666-666666666666', amount: created ? 300000 : 150000, provider: body.provider, status: 'pending', pay_url: 'https://payment.example.test/checkout', expires_at: paymentExpiresAt });
    if (path === '/payments') return reply([]);
    return reply({ error: { code: 'UNMOCKED_ROUTE', message: `Unmocked ${method} ${path}` } }, 500);
  });
  await page.context().route('https://payment.example.test/**', (route) => route.fulfill({ contentType: 'text/html', body: '<title>Fixture checkout</title><p>Payment provider fixture</p>' }));
  return { calls, setPaid: () => { paid = true; } };
}

export async function login(page: Page, next = '/') {
  await page.goto(`/login?next=${encodeURIComponent(next)}`);
  await page.getByLabel('Email', { exact: true }).fill('player@example.test');
  await page.getByLabel('Mật khẩu', { exact: true }).fill('SecurePassword123!');
  await page.getByRole('button', { name: 'Đăng nhập', exact: true }).last().click();
}
