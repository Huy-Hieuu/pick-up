import React, { useCallback, useEffect, useState } from "react";
import {
  AppState,
  Linking,
  Platform,
  Share,
  View,
  useWindowDimensions,
  type DimensionValue,
} from "react-native";
import { useFocusEffect, useLocalSearchParams } from "expo-router";
import type { GameDetail, GameStatus, Payment } from "../../src/types/api";
import { useResource } from "../../src/hooks/useResource";
import { useActionScope } from "../../src/hooks/useActionScope";
import { isGameDetail, isPayment } from "../../src/types/validators";
import { useSession } from "../../src/stores/session";
import {
  day,
  gameStatus,
  money,
  safeWebUrl,
  sportName,
  time,
} from "../../src/utils/format";
import {
  AppText,
  AuthGate,
  BackLink,
  Button,
  Card,
  Icon,
  Message,
  Page,
  Pill,
  StateBlock,
  palette,
  type,
  ui,
} from "../../src/components/ui";
import { formError } from "../../src/components/AuthFrame";

export default function GameScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { session, ready, request } = useSession();
  const { width } = useWindowDimensions();
  const valid =
    typeof id === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id);
  const identity = id + ":" + (session?.user.id || "guest");
  const capture = useActionScope(identity);
  const resource = useResource<GameDetail>(
    session && valid ? "/games/" + id : null,
    isGameDetail,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [provider, setProvider] = useState<"momo" | "zalopay">("momo");
  const [payment, setPayment] = useState<Payment | null>(null);
  const [confirm, setConfirm] = useState<"leave" | "cancel" | null>(null);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    setBusy(false);
    setError(null);
    setNotice(null);
    setProvider("momo");
    setPayment(null);
    setConfirm(null);
  }, [identity]);
  useEffect(() => {
    if (!payment || payment.status !== "pending" || !payment.expires_at) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [payment]);
  useEffect(() => {
    const game = resource.data;
    if (!game) return;
    const bill = game.split?.payments.find(
      (entry) => entry.user_id === session?.user.id,
    );
    const status = game.players.find(
      (entry) => entry.user_id === session?.user.id,
    )?.payment_status;
    if (
      game.status === "cancelled" ||
      !bill ||
      bill.paid ||
      (payment && bill.amount !== payment.amount) ||
      status === "expired" ||
      status === "refunded" ||
      status === "disputed"
    )
      setPayment(null);
    // Reconcile when fresh game data arrives; the pre-initiation snapshot may be expired.
  }, [resource.data, session?.user.id]);
  useFocusEffect(
    useCallback(() => {
      setBusy(false);
      if (!session || !valid) return;
      const refreshVisible = () => {
        if (
          Platform.OS === "web"
            ? typeof document !== "undefined" &&
              document.visibilityState === "visible"
            : AppState.currentState === "active"
        )
          resource.refresh();
      };
      refreshVisible();
      const timer = setInterval(refreshVisible, 15000);
      const listener = AppState.addEventListener("change", (state) => {
        if (state === "active") refreshVisible();
      });
      if (Platform.OS === "web" && typeof document !== "undefined")
        document.addEventListener("visibilitychange", refreshVisible);
      return () => {
        clearInterval(timer);
        listener.remove();
        if (Platform.OS === "web" && typeof document !== "undefined")
          document.removeEventListener("visibilitychange", refreshVisible);
      };
    }, [resource.refresh, session?.user.id, valid]),
  );
  async function mutate(
    action: "join" | "leave" | "cancel",
    status?: GameStatus,
  ) {
    if (busy) return;
    const current = capture();
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await request("/games/" + id + (status ? "/status" : "/" + action), {
        method: status ? "PATCH" : "POST",
        ...(status ? { body: { status } } : {}),
      });
      if (!current()) return;
      setConfirm(null);
      setNotice(
        status
          ? "Đã cập nhật trạng thái trận."
          : action === "join"
            ? "Bạn đã tham gia trận."
            : action === "leave"
              ? "Bạn đã rời trận."
              : "Đã hủy trận.",
      );
      resource.refresh();
    } catch (reason) {
      if (current()) {
        setError(formError(reason));
        resource.refresh();
      }
    } finally {
      if (current()) setBusy(false);
    }
  }
  async function pay() {
    if (busy) return;
    const current = capture();
    setBusy(true);
    setError(null);
    setPayment(null);
    try {
      const value = await request<Payment>(
        "/games/" + id + "/payments/initiate",
        { method: "POST", body: { provider } },
      );
      if (!current()) return;
      if (!isPayment(value) || value.provider !== provider)
        throw new Error(
          "Chưa xác nhận được yêu cầu thanh toán. Vui lòng tải lại trước khi thử tiếp.",
        );
      setNow(Date.now());
      setPayment(value);
      resource.refresh();
    } catch (reason) {
      if (current()) setError(formError(reason));
    } finally {
      if (current()) setBusy(false);
    }
  }
  async function share() {
    const current = capture();
    try {
      const value = await request<{ url: string; message: string }>(
        "/games/" + id + "/share",
      );
      if (!current()) return;
      const url = safeWebUrl(value?.url);
      if (!url || typeof value?.message !== "string")
        throw new Error("Chưa tạo được liên kết chia sẻ.");
      if (Platform.OS === "web") {
        const text = value.message + "\n" + url;
        if (navigator.clipboard?.writeText) {
          await navigator.clipboard.writeText(text);
          setNotice("Đã sao chép liên kết chia sẻ trận.");
        } else {
          setNotice(text);
        }
        return;
      }
      await Share.share({ message: value.message + "\n" + url });
    } catch (reason) {
      if (current()) setError(formError(reason));
    }
  }
  if (!ready)
    return (
      <Page>
        <StateBlock loading />
      </Page>
    );
  if (!session)
    return (
      <Page>
        <BackLink />
        <AuthGate next={"/game/" + id}>
          Đăng nhập để xem trận, tham gia và thanh toán phần tiền của bạn.
        </AuthGate>
      </Page>
    );
  if (!valid)
    return (
      <Page>
        <StateBlock empty="Đường dẫn trận không hợp lệ." />
      </Page>
    );
  const game = resource.data;
  if (!game)
    return (
      <Page>
        <BackLink />
        <StateBlock
          loading={resource.loading}
          error={resource.error}
          retry={resource.refresh}
          empty="Không tìm thấy trận này."
        />
      </Page>
    );
  const creator = game.creator_id === session.user.id;
  const ownPlayer = game.players.find(
    (player) => player.user_id === session.user.id,
  );
  const member = !!ownPlayer;
  const split = game.split;
  const bill = split?.payments.find(
    (entry) => entry.user_id === session.user.id,
  );
  const wide = width >= 850;
  const disputed = ownPlayer?.payment_status === "disputed";
  const payable =
    !!bill && !bill.paid && game.status !== "cancelled" && !disputed;
  const expired =
    !!payment?.expires_at && Date.parse(payment.expires_at) <= now;
  const payUrl =
    !expired && payment?.provider === provider
      ? safeWebUrl(payment?.pay_url || null)
      : null;
  const changeable = game.status === "open" || game.status === "full";
  const paidWidth = ((split && split.total_amount > 0
    ? Math.max(0, Math.min(100, (split.paid_total / split.total_amount) * 100))
    : 0) + "%") as DimensionValue;
  return (
    <Page>
      <BackLink title="Về trận đấu" />
      <View style={{ gap: 13 }}>
        <View style={[ui.row, { flexWrap: "wrap" }]}>
          <AppText style={ui.eyebrow}>
            {sportName(game.sport_type).toUpperCase()}
          </AppText>
          <Pill title={gameStatus[game.status]} />
        </View>
        <AppText accessibilityRole="header" style={type.title}>
          {game.court.name}
        </AppText>
        <View style={ui.row}>
          <Icon name="calendar" size={17} />
          <AppText>
            {day(game.slot.start_time)} · {time(game.slot.start_time)}–
            {time(game.slot.end_time)}
          </AppText>
        </View>
        <View style={ui.row}>
          <Icon name="map-pin" size={17} />
          <AppText style={{ flex: 1, color: palette.muted }}>
            {game.court.address}
          </AppText>
        </View>
      </View>
      {resource.error && (
        <>
          <Message error>{formError(resource.error)}</Message>
          <Button
            title="Thử tải lại trận"
            secondary
            onPress={resource.refresh}
          />
        </>
      )}
      {error && <Message error>{error}</Message>}
      {notice && <Message>{notice}</Message>}
      <View
        style={{
          flexDirection: wide ? "row" : "column",
          gap: 24,
          alignItems: wide ? "flex-start" : "stretch",
        }}
      >
        <View style={{ flex: 1, gap: 24 }}>
          <Card>
            <View style={[ui.row, { justifyContent: "space-between" }]}>
              <AppText style={type.heading}>Đồng đội</AppText>
              <AppText>
                {game.players.length}/{game.max_players}
              </AppText>
            </View>
            {game.description && (
              <AppText style={{ color: palette.muted }}>
                {game.description}
              </AppText>
            )}
            {game.players.map((player) => (
              <View
                key={player.user_id}
                style={[ui.row, { justifyContent: "space-between" }]}
              >
                <View style={[ui.row, { flex: 1 }]}>
                  <View
                    style={{
                      backgroundColor: palette.pale,
                      width: 40,
                      height: 40,
                      borderRadius: 20,
                      alignItems: "center",
                      justifyContent: "center",
                    }}
                  >
                    <AppText style={{ fontFamily: "Inter_600SemiBold" }}>
                      {(player.display_name || "P").slice(0, 1).toUpperCase()}
                    </AppText>
                  </View>
                  <AppText style={{ flexShrink: 1 }}>
                    {player.display_name || "Người chơi"}
                    {player.user_id === session.user.id ? " (Bạn)" : ""}
                  </AppText>
                </View>
                <AppText style={{ color: palette.muted, fontSize: 11 }}>
                  {player.user_id === game.creator_id ? "Chủ trận" : "Đồng đội"}
                </AppText>
              </View>
            ))}
            {!member &&
              game.status === "open" &&
              Date.parse(game.slot.start_time) > Date.now() && (
                <Button
                  title="Tham gia trận"
                  icon="users"
                  loading={busy}
                  onPress={() => {
                    void mutate("join");
                  }}
                />
              )}
            {member && !creator && changeable && (
              <Button
                title="Rời trận"
                secondary
                disabled={busy}
                onPress={() => setConfirm("leave")}
              />
            )}
            <Button
              title="Chia sẻ trận"
              secondary
              icon="share-2"
              onPress={() => {
                void share();
              }}
            />
          </Card>
          {split && (
            <Card>
              <AppText style={type.heading}>Chia tiền minh bạch</AppText>
              <View style={[ui.row, { justifyContent: "space-between" }]}>
                <AppText style={{ color: palette.muted }}>
                  Tổng tiền sân
                </AppText>
                <AppText style={{ fontFamily: "Inter_600SemiBold" }}>
                  {money(split.total_amount)}
                </AppText>
              </View>
              <View
                accessibilityRole="progressbar"
                accessibilityValue={{
                  min: 0,
                  max: split.total_amount,
                  now: Math.min(split.paid_total, split.total_amount),
                }}
                accessibilityLabel="Tiền sân đã thanh toán"
                style={{
                  height: 8,
                  borderRadius: 4,
                  backgroundColor: palette.pale,
                  overflow: "hidden",
                }}
              >
                <View
                  style={{
                    height: "100%",
                    width: paidWidth,
                    backgroundColor: palette.green,
                  }}
                />
              </View>
              <AppText style={{ color: palette.muted, fontSize: 12 }}>
                Đã thu {money(split.paid_total)} · Còn{" "}
                {money(split.remaining_total)}
              </AppText>
              <View style={ui.divider} />
              {split.payments.map((entry) => (
                <View
                  key={entry.user_id}
                  style={[
                    ui.row,
                    { justifyContent: "space-between", flexWrap: "wrap" },
                  ]}
                >
                  <AppText style={{ flexShrink: 1 }}>
                    {entry.display_name || "Người chơi"}
                    {entry.user_id === session.user.id ? " (Bạn)" : ""}
                  </AppText>
                  <View style={ui.row}>
                    <AppText>{money(entry.amount)}</AppText>
                    <Icon
                      name={entry.paid ? "check-circle" : "clock"}
                      size={16}
                      color={entry.paid ? palette.green : palette.muted}
                    />
                  </View>
                </View>
              ))}
              {split.creator_pays_extra && (
                <AppText style={{ fontSize: 11, color: palette.muted }}>
                  Chủ trận chịu phần tiền làm tròn để tổng các phần luôn bằng
                  tiền sân.
                </AppText>
              )}
            </Card>
          )}
        </View>
        <View style={{ width: wide ? 350 : "100%", gap: 20 }}>
          <Card>
            <AppText style={type.heading}>
              {bill
                ? bill.paid
                  ? "Phần của bạn đã thanh toán"
                  : "Phần tiền của bạn"
                : "Một trận, nhiều niềm vui"}
            </AppText>
            {bill ? (
              <AppText style={{ fontFamily: "Inter_700Bold", fontSize: 32 }}>
                {money(bill.amount)}
              </AppText>
            ) : (
              <AppText style={{ color: palette.muted }}>
                Tham gia để xem phần tiền của bạn.
              </AppText>
            )}
            {bill?.paid && (
              <Message>
                Đã xác nhận thanh toán. Bạn không cần trả thêm cho phần này.
              </Message>
            )}
            {disputed && (
              <Message error>
                Yêu cầu thanh toán đang được đối soát. Vui lòng liên hệ chủ trận
                trước khi thanh toán tiếp.
              </Message>
            )}
            {payable && (
              <>
                <View style={[ui.row, { flexWrap: "wrap" }]}>
                  <Pill
                    title="MoMo"
                    selected={provider === "momo"}
                    onPress={
                      busy
                        ? undefined
                        : () => {
                            setProvider("momo");
                            setPayment(null);
                          }
                    }
                  />
                  <Pill
                    title="ZaloPay"
                    selected={provider === "zalopay"}
                    onPress={
                      busy
                        ? undefined
                        : () => {
                            setProvider("zalopay");
                            setPayment(null);
                          }
                    }
                  />
                </View>
                <Button
                  title="Thanh toán phần của tôi"
                  icon="arrow-up-right"
                  loading={busy}
                  onPress={() => {
                    void pay();
                  }}
                />
                {payment && (
                  <Message>
                    {payment.status === "paid"
                      ? "Đã xác nhận thanh toán."
                      : expired || payment.status === "expired"
                        ? "Liên kết thanh toán đã hết hạn. Tải lại trạng thái trước khi tạo yêu cầu mới."
                        : "Đơn thanh toán đã được tạo. Mở trang nhà cung cấp để tiếp tục."}
                  </Message>
                )}
                {payUrl && payment?.status === "pending" && (
                  <Button
                    title="Mở trang thanh toán"
                    secondary
                    icon="external-link"
                    onPress={() => {
                      if (
                        payment?.expires_at &&
                        Date.parse(payment.expires_at) <= Date.now()
                      ) {
                        setNow(Date.now());
                        return;
                      }
                      const current = capture();
                      void Linking.openURL(payUrl).catch(() => {
                        if (current())
                          setError(
                            "Không mở được trang thanh toán. Vui lòng thử lại.",
                          );
                      });
                    }}
                  />
                )}
                <AppText style={{ fontSize: 11, color: palette.muted }}>
                  Trạng thái cập nhật khi nhà cung cấp xác nhận. Nếu đang chờ xử
                  lý, tải lại để kiểm tra trước khi thử tiếp.
                </AppText>
              </>
            )}
            <Button
              title="Tải lại trạng thái"
              secondary
              icon="refresh-cw"
              disabled={resource.loading}
              onPress={resource.refresh}
            />
          </Card>
          {creator &&
            game.status !== "cancelled" &&
            game.status !== "completed" && (
              <Card>
                <AppText style={type.subheading}>Quản lý trận</AppText>
                {changeable && (
                  <Button
                    title="Bắt đầu trận"
                    secondary
                    loading={busy}
                    onPress={() => {
                      void mutate("join", "in_progress");
                    }}
                  />
                )}
                {game.status === "in_progress" && (
                  <Button
                    title="Hoàn tất trận"
                    secondary
                    loading={busy}
                    onPress={() => {
                      void mutate("join", "completed");
                    }}
                  />
                )}
                <Button
                  title="Hủy trận"
                  danger
                  disabled={busy}
                  onPress={() => setConfirm("cancel")}
                />
              </Card>
            )}
          {confirm && (
            <Card>
              <AppText>
                {confirm === "leave"
                  ? "Rời trận này? Phần tiền của các thành viên có thể thay đổi."
                  : "Hủy trận và trả lại sân? Các trận có tiền đã thu cần xử lý theo chính sách thanh toán."}
              </AppText>
              <Button
                title={
                  confirm === "leave"
                    ? "Xác nhận rời trận"
                    : "Xác nhận hủy trận"
                }
                danger
                loading={busy}
                onPress={() => {
                  void mutate(confirm);
                }}
              />
              <Button
                title="Giữ nguyên"
                secondary
                disabled={busy}
                onPress={() => setConfirm(null)}
              />
            </Card>
          )}
        </View>
      </View>
    </Page>
  );
}
