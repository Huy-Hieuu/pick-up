import React, { useCallback, useEffect, useState } from "react";
import { Linking, Pressable, View, useWindowDimensions } from "react-native";
import { Image } from "expo-image";
import { router, useFocusEffect, useLocalSearchParams } from "expo-router";
import type { Booking, Court, SlotList } from "../../src/types/api";
import { useResource } from "../../src/hooks/useResource";
import { useActionScope } from "../../src/hooks/useActionScope";
import { isBooking, isCourt, isSlotList } from "../../src/types/validators";
import { useSession } from "../../src/stores/session";
import {
  dateKey,
  day,
  money,
  safeWebUrl,
  sportName,
  time,
} from "../../src/utils/format";
import {
  AppText,
  BackLink,
  Button,
  Card,
  Icon,
  IconButton,
  Message,
  Page,
  Pill,
  StateBlock,
  palette,
  type,
  ui,
} from "../../src/components/ui";
import { formError } from "../../src/components/AuthFrame";

export default function CourtDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { width } = useWindowDimensions();
  const { session, request, saved, toggleSaved } = useSession();
  const valid =
    typeof id === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id);
  const identity = id + ":" + (session?.user.id || "guest");
  const capture = useActionScope(identity);
  const [date, setDate] = useState(dateKey());
  const [selected, setSelected] = useState<string | null>(null);
  const [booking, setBooking] = useState<Booking | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmCancel, setConfirmCancel] = useState(false);
  const [photoFailed, setPhotoFailed] = useState(false);
  const court = useResource<Court>(valid ? "/courts/" + id : null, isCourt);
  const slots = useResource<SlotList>(
    valid ? "/courts/" + id + "/slots?date=" + date : null,
    isSlotList,
  );
  useEffect(() => {
    setDate(dateKey());
    setSelected(null);
    setBooking(null);
    setBusy(false);
    setError(null);
    setConfirmCancel(false);
  }, [identity]);
  useFocusEffect(
    useCallback(() => {
      setBusy(false);
      court.refresh();
      slots.refresh();
    }, [court.refresh, slots.refresh]),
  );
  const data = court.data;
  const photo = court.data?.photo_urls.map(safeWebUrl).find(Boolean);
  useEffect(() => setPhotoFailed(false), [photo]);
  const list = Array.isArray(slots.data?.slots) ? slots.data.slots : [];
  const chosen = list.find(
    (slot) =>
      slot.id === selected &&
      slot.status === "available" &&
      Date.parse(slot.start_time) > Date.now(),
  );
  async function book() {
    if (!chosen || busy) return;
    if (!session) {
      router.push({ pathname: "/login", params: { next: "/court/" + id } });
      return;
    }
    const current = capture();
    setBusy(true);
    setError(null);
    try {
      const result = await request<Booking>(
        "/courts/" + id + "/slots/" + chosen.id + "/book",
        { method: "POST", body: {} },
      );
      if (!current()) return;
      if (
        !isBooking(result) ||
        result.court_id !== id ||
        result.slot_id !== chosen.id ||
        result.booked_by !== session.user.id ||
        result.status !== "booked"
      )
        throw new Error(
          "Chưa xác nhận được booking. Kiểm tra lịch của bạn trước khi thử lại.",
        );
      setBooking(result);
      setSelected(null);
      slots.refresh();
    } catch (reason) {
      if (current()) {
        setError(formError(reason));
        slots.refresh();
      }
    } finally {
      if (current()) setBusy(false);
    }
  }
  async function cancel() {
    if (!booking || busy || booking.court_id !== id) return;
    const current = capture();
    setBusy(true);
    setError(null);
    try {
      await request("/courts/" + id + "/slots/" + booking.slot_id + "/cancel", {
        method: "POST",
      });
      if (current()) {
        setBooking(null);
        setConfirmCancel(false);
        slots.refresh();
      }
    } catch (reason) {
      if (current()) setError(formError(reason));
    } finally {
      if (current()) setBusy(false);
    }
  }
  if (!valid)
    return (
      <Page>
        <StateBlock empty="Đường dẫn sân không hợp lệ." />
        <Button title="Khám phá sân" onPress={() => router.replace("/")} />
      </Page>
    );
  if (!data)
    return (
      <Page>
        <BackLink />
        <StateBlock
          loading={court.loading}
          error={court.error}
          retry={court.refresh}
          empty="Không tìm thấy sân này."
        />
      </Page>
    );
  const wide = width >= 850;
  return (
    <Page>
      <BackLink title="Về danh sách sân" />
      {court.error && (
        <>
          <Message error>{formError(court.error)}</Message>
          <Button
            title="Tải lại thông tin sân"
            secondary
            onPress={court.refresh}
          />
        </>
      )}
      <View style={{ gap: 13 }}>
        <View style={[ui.row, { justifyContent: "space-between" }]}>
          <AppText style={ui.eyebrow}>
            {sportName(data.sport_type).toUpperCase()}
          </AppText>
          <IconButton
            name="heart"
            active={saved.includes(id)}
            label={saved.includes(id) ? "Bỏ lưu sân" : "Lưu sân"}
            onPress={() => toggleSaved(id)}
          />
        </View>
        <AppText accessibilityRole="header" style={type.title}>
          {data.name}
        </AppText>
        <View style={ui.row}>
          <Icon name="map-pin" size={16} />
          <AppText style={{ color: palette.muted, flex: 1 }}>
            {data.address}
          </AppText>
        </View>
      </View>
      <View
        style={{
          borderRadius: 22,
          overflow: "hidden",
          height: wide ? 330 : 220,
          backgroundColor: palette.pale,
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {photo && !photoFailed ? (
          <Image
            source={{ uri: photo }}
            contentFit="cover"
            style={{ width: "100%", height: "100%" }}
            accessibilityLabel={data.name}
            onError={() => setPhotoFailed(true)}
          />
        ) : (
          <View style={{ alignItems: "center", gap: 15 }}>
            <Icon name="sun" color={palette.green} size={55} />
            <AppText style={type.heading}>{sportName(data.sport_type)}</AppText>
          </View>
        )}
      </View>
      <View
        style={{
          flexDirection: wide ? "row" : "column",
          gap: 24,
          alignItems: wide ? "flex-start" : "stretch",
        }}
      >
        <View style={{ flex: 1, gap: 20 }}>
          <AppText style={type.heading}>Chọn khung giờ của bạn</AppText>
          <View style={[ui.row, { flexWrap: "wrap" }]}>
            {Array.from({ length: 7 }, (_, offset) => {
              const value = dateKey(offset);
              return (
                <Pill
                  key={value}
                  title={
                    offset === 0 ? "Hôm nay" : day(value + "T12:00:00+07:00")
                  }
                  selected={date === value}
                  onPress={() => {
                    setDate(value);
                    setSelected(null);
                    setError(null);
                  }}
                />
              );
            })}
          </View>
          {slots.error && slots.data && (
            <>
              <Message error>{formError(slots.error)}</Message>
              <Button
                title="Tải lại khung giờ"
                secondary
                onPress={slots.refresh}
              />
            </>
          )}
          {slots.loading && !slots.data ? (
            <StateBlock loading />
          ) : slots.error && !slots.data ? (
            <StateBlock error={slots.error} retry={slots.refresh} />
          ) : list.length ? (
            <View style={[ui.row, { flexWrap: "wrap" }]}>
              {list.map((slot) => {
                const available =
                  slot.status === "available" &&
                  Date.parse(slot.start_time) > Date.now();
                return (
                  <Pressable
                    key={slot.id}
                    accessibilityRole="button"
                    accessibilityLabel={
                      "Chọn khung giờ " +
                      time(slot.start_time) +
                      "–" +
                      time(slot.end_time)
                    }
                    accessibilityState={{
                      selected: slot.id === selected,
                      disabled: !available,
                    }}
                    disabled={!available || busy}
                    onPress={() => setSelected(slot.id)}
                    style={{
                      minHeight: 66,
                      minWidth: 130,
                      padding: 15,
                      gap: 4,
                      borderRadius: 12,
                      borderWidth: 1,
                      borderColor:
                        slot.id === selected ? palette.green : palette.line,
                      backgroundColor:
                        slot.id === selected ? palette.pale : palette.white,
                      opacity: available ? 1 : 0.45,
                    }}
                  >
                    <AppText
                      style={{ fontFamily: "Inter_600SemiBold", fontSize: 13 }}
                    >
                      {time(slot.start_time)}–{time(slot.end_time)}
                    </AppText>
                    <AppText style={{ fontSize: 10, color: palette.muted }}>
                      {available ? "Còn trống" : "Không khả dụng"}
                    </AppText>
                  </Pressable>
                );
              })}
            </View>
          ) : (
            <StateBlock empty="Sân chưa có khung giờ cho ngày này. Hãy chọn ngày khác." />
          )}
          <View style={ui.divider} />
          <Button
            title="Xem vị trí trên bản đồ"
            secondary
            icon="map-pin"
            disabled={!Number.isFinite(data.lat) || !Number.isFinite(data.lng)}
            onPress={() => {
              void Linking.openURL(
                "https://www.google.com/maps/search/?api=1&query=" +
                  data.lat +
                  "," +
                  data.lng,
              ).catch(() =>
                setError("Không mở được bản đồ. Vui lòng thử lại."),
              );
            }}
          />
        </View>
        <Card style={{ width: wide ? 350 : "100%" }}>
          <AppText style={type.heading}>{money(data.price_per_slot)}</AppText>
          <AppText style={{ color: palette.muted }}>
            Mỗi khung giờ · Chia tiền khi tạo trận
          </AppText>
          {error && <Message error>{error}</Message>}
          {booking ? (
            <>
              <Message>Đã đặt sân thành công.</Message>
              <AppText>
                {day(booking.start_time)} · {time(booking.start_time)}–
                {time(booking.end_time)}
              </AppText>
              <Button
                title="Tạo trận từ lịch này"
                icon="users"
                onPress={() =>
                  router.push({
                    pathname: "/game/create",
                    params: { slot: booking.slot_id, court: id },
                  })
                }
              />
              <Button
                title="Xem lịch của tôi"
                secondary
                onPress={() => router.push("/profile")}
              />
              {confirmCancel ? (
                <>
                  <AppText>
                    Hủy booking này và trả lại khung giờ cho người khác?
                  </AppText>
                  <Button
                    title="Xác nhận hủy lịch"
                    danger
                    loading={busy}
                    onPress={() => {
                      void cancel();
                    }}
                  />
                  <Button
                    title="Giữ lịch này"
                    secondary
                    disabled={busy}
                    onPress={() => setConfirmCancel(false)}
                  />
                </>
              ) : (
                <Button
                  title="Hủy lịch đã đặt"
                  danger
                  onPress={() => setConfirmCancel(true)}
                />
              )}
            </>
          ) : (
            <>
              {chosen ? (
                <AppText>
                  {day(chosen.start_time)} · {time(chosen.start_time)}–
                  {time(chosen.end_time)}
                </AppText>
              ) : (
                <AppText style={{ color: palette.muted }}>
                  Chọn một khung giờ còn trống để tiếp tục.
                </AppText>
              )}
              <Button
                title={session ? "Đặt sân" : "Đăng nhập để đặt sân"}
                loading={busy}
                disabled={!chosen}
                onPress={() => {
                  void book();
                }}
              />
              <AppText style={{ fontSize: 11, color: palette.muted }}>
                Bạn có thể tạo trận và mời đồng đội sau khi đặt sân. Booking
                luôn được kiểm tra lại trước khi xác nhận.
              </AppText>
            </>
          )}
        </Card>
      </View>
    </Page>
  );
}
