import React, { useCallback, useEffect, useRef, useState } from "react";
import { Platform, View, useWindowDimensions } from "react-native";
import { Image } from "expo-image";
import * as ImagePicker from "expo-image-picker";
import { router, useFocusEffect } from "expo-router";
import type {
  GamePage,
  PersonalBooking,
  PersonalBookingPage,
  User,
} from "../../src/types/api";
import { useResource } from "../../src/hooks/useResource";
import { useActionScope } from "../../src/hooks/useActionScope";
import {
  isGamePage,
  isPersonalBookingPage,
  isUser,
} from "../../src/types/validators";
import { useSession } from "../../src/stores/session";
import {
  day,
  money,
  safeWebUrl,
  sportName,
  time,
} from "../../src/utils/format";
import {
  AppText,
  AuthGate,
  Button,
  Card,
  Field,
  Icon,
  Message,
  Page,
  SectionTitle,
  StateBlock,
  palette,
  type,
  ui,
} from "../../src/components/ui";
import { formError } from "../../src/components/AuthFrame";
import { GameCard } from "../../src/components/GameCard";

export default function Profile() {
  const { session, ready, request, logout, updateUser } = useSession();
  const { width } = useWindowDimensions();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [gamePage, setGamePage] = useState(1);
  const [confirm, setConfirm] = useState<PersonalBooking | null>(null);
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const [avatarBusy, setAvatarBusy] = useState(false);
  const [avatarFailed, setAvatarFailed] = useState(false);
  const account = session?.user.id || "guest";
  const capture = useActionScope(account);
  const captureScreen = useActionScope("profile");
  const latestAccount = useRef(session?.user.id);
  latestAccount.current = session?.user.id;
  const profile = useResource<User>(session ? "/auth/me" : null, isUser);
  const bookings = useResource<PersonalBookingPage>(
    session ? "/auth/me/bookings?page=" + page + "&per_page=6" : null,
    isPersonalBookingPage,
  );
  const games = useResource<GamePage>(
    session ? "/auth/me/games?page=" + gamePage + "&per_page=6" : null,
    isGamePage,
  );
  useEffect(() => {
    setName("");
    setBusy(false);
    setAvatarBusy(false);
    setError(null);
    setNotice(null);
    setPage(1);
    setGamePage(1);
    setConfirm(null);
    if (account !== "guest") setLogoutError(null);
  }, [account]);
  useFocusEffect(
    useCallback(() => {
      setBusy(false);
      setAvatarBusy(false);
      profile.refresh();
      bookings.refresh();
      games.refresh();
    }, [profile.refresh, bookings.refresh, games.refresh]),
  );
  useEffect(() => {
    if (profile.data) setName(profile.data.display_name || "");
  }, [profile.data?.id, profile.data?.display_name]);
  async function save() {
    if (busy) return;
    const current = capture();
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const user = await request<User>("/auth/me", {
        method: "PATCH",
        body: { display_name: name.trim() || null },
      });
      if (!current()) return;
      if (!isUser(user) || user.id !== account)
        throw new Error("Máy chủ trả về hồ sơ không hợp lệ. Vui lòng tải lại.");
      await updateUser(user);
      if (current()) {
        setNotice("Đã lưu hồ sơ.");
        profile.refresh();
      }
    } catch (reason) {
      if (current()) setError(formError(reason));
    } finally {
      if (current()) setBusy(false);
    }
  }
  async function signOut() {
    if (busy) return;
    const current = captureScreen();
    setBusy(true);
    setError(null);
    setLogoutError(null);
    try {
      await logout();
      if (current() && !latestAccount.current) router.replace("/");
    } catch (reason) {
      if (current() && !latestAccount.current)
        setLogoutError(
          "Đã đăng xuất trên thiết bị. Chưa xác nhận được việc thu hồi phiên trên máy chủ. " +
            formError(reason),
        );
    } finally {
      if (current() && !latestAccount.current) setBusy(false);
    }
  }
  async function cancelBooking() {
    if (busy || !confirm) return;
    const current = capture();
    setBusy(true);
    setError(null);
    try {
      await request(
        "/courts/" + confirm.court_id + "/slots/" + confirm.slot_id + "/cancel",
        { method: "POST" },
      );
      if (current()) {
        setConfirm(null);
        setNotice("Đã hủy booking.");
        bookings.refresh();
      }
    } catch (reason) {
      if (current()) setError(formError(reason));
    } finally {
      if (current()) setBusy(false);
    }
  }
  async function changeAvatar(remove = false) {
    if (busy || !session) return;
    const current = capture();
    setBusy(true);
    setAvatarBusy(true);
    setError(null);
    setNotice(null);
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      let avatarUrl: string | null = null;
      if (!remove) {
        if (Platform.OS !== "web") {
          const permission =
            await ImagePicker.requestMediaLibraryPermissionsAsync();
          if (!current()) return;
          if (!permission.granted)
            throw new Error(
              "Hãy cho phép truy cập ảnh trong cài đặt để chọn ảnh đại diện.",
            );
        }
        const result = await ImagePicker.launchImageLibraryAsync({
          mediaTypes: ["images"],
          allowsEditing: true,
          aspect: [1, 1],
          quality: 0.85,
        });
        if (!current() || result.canceled) return;
        const asset = result.assets[0];
        if (!asset || (asset.fileSize && asset.fileSize > 8 * 1024 * 1024))
          throw new Error(
            "Chọn ảnh JPEG, PNG hoặc WebP có dung lượng tối đa 8 MB.",
          );
        timer = setTimeout(() => controller.abort(), 30000);
        const blob =
          asset.file ||
          (await (
            await fetch(asset.uri, { signal: controller.signal })
          ).blob());
        if (!current()) return;
        const mime = blob.type || asset.mimeType || "";
        if (
          !["image/jpeg", "image/png", "image/webp"].includes(mime) ||
          blob.size === 0 ||
          blob.size > 8 * 1024 * 1024
        )
          throw new Error(
            "Chọn ảnh JPEG, PNG hoặc WebP có dung lượng tối đa 8 MB.",
          );
        const upload = await request<{
          upload_id: string;
          upload_url: string;
          content_length: number;
        }>("/upload/avatar", {
          method: "POST",
          body: { content_type: mime, content_length: blob.size },
          signal: controller.signal,
        });
        if (!current()) return;
        const url = safeWebUrl(upload?.upload_url);
        if (
          !url ||
          !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
            upload?.upload_id || "",
          ) ||
          upload.content_length !== blob.size
        )
          throw new Error(
            "Chưa nhận được địa chỉ tải ảnh hợp lệ. Vui lòng thử lại.",
          );
        const response = await fetch(url, {
          method: "PUT",
          headers: { "Content-Type": mime },
          body: blob,
          signal: controller.signal,
        });
        if (!current()) return;
        if (!response.ok)
          throw new Error("Chưa tải được ảnh. Vui lòng thử lại.");
        const completed = await request<{ public_url: string }>(
          "/upload/avatar/complete",
          {
            method: "POST",
            body: { upload_id: upload.upload_id },
            signal: controller.signal,
          },
        );
        if (!current()) return;
        avatarUrl = safeWebUrl(completed?.public_url) || null;
        if (!avatarUrl)
          throw new Error(
            "Máy chủ chưa xác nhận ảnh đại diện. Vui lòng thử lại.",
          );
      }
      const user = await request<User>("/auth/me", {
        method: "PATCH",
        body: { avatar_url: avatarUrl },
        signal: controller.signal,
      });
      if (!current()) return;
      if (!isUser(user) || user.id !== account)
        throw new Error("Máy chủ trả về hồ sơ không hợp lệ. Vui lòng tải lại.");
      await updateUser(user);
      if (current()) {
        setNotice(
          remove ? "Đã xóa ảnh đại diện." : "Đã cập nhật ảnh đại diện.",
        );
        profile.refresh();
      }
    } catch (reason) {
      if (current()) setError(formError(reason));
    } finally {
      if (timer) clearTimeout(timer);
      if (current()) {
        setBusy(false);
        setAvatarBusy(false);
      }
    }
  }
  const avatar = safeWebUrl(
    profile.data ? profile.data.avatar_url : session?.user.avatar_url ?? null,
  );
  useEffect(() => setAvatarFailed(false), [avatar]);
  const columns = width >= 1000 ? 3 : width >= 670 ? 2 : 1;
  const cardWidth =
    (Math.min(width, 1240) - (width < 600 ? 40 : 80) - (columns - 1) * 20) /
    columns;
  if (!ready)
    return (
      <Page>
        <StateBlock loading />
      </Page>
    );
  if (!session)
    return (
      <Page>
        <AppText accessibilityRole="header" style={type.title}>
          Tài khoản của bạn.
        </AppText>
        {logoutError && <Message error>{logoutError}</Message>}
        <AuthGate next="/profile">
          Đăng nhập để xem lịch đặt sân, các trận đã tham gia và quản lý hồ sơ.
        </AuthGate>
      </Page>
    );
  return (
    <Page>
      <AppText style={ui.eyebrow}>HẸN NHAU Ở SÂN</AppText>
      <AppText accessibilityRole="header" style={type.title}>
        Chào {session.user.display_name || "bạn"}.
      </AppText>
      {error && <Message error>{error}</Message>}
      {notice && <Message>{notice}</Message>}
      <Card style={{ maxWidth: 680, width: "100%" }}>
        <View style={ui.row}>
          <View
            style={{
              height: 64,
              width: 64,
              borderRadius: 32,
              backgroundColor: palette.pale,
              alignItems: "center",
              justifyContent: "center",
              overflow: "hidden",
            }}
          >
            {avatar && !avatarFailed ? (
              <Image
                source={{ uri: avatar }}
                style={{ width: "100%", height: "100%" }}
                contentFit="cover"
                accessibilityLabel="Ảnh đại diện của bạn"
                onError={() => setAvatarFailed(true)}
              />
            ) : (
              <Icon name="user" color={palette.green} size={27} />
            )}
          </View>
          <View style={{ flex: 1 }}>
            <AppText style={type.subheading}>
              {session.user.display_name || "Người chơi PickUp"}
            </AppText>
            <AppText style={{ color: palette.muted, fontSize: 12 }}>
              {session.user.email}
            </AppText>
          </View>
        </View>
        {profile.loading && !profile.data && (
          <AppText accessibilityLiveRegion="polite" style={{ color: palette.muted }}>
            Đang tải hồ sơ…
          </AppText>
        )}
        {profile.error && (
          <>
            <Message error>{formError(profile.error)}</Message>
            <Button title="Tải lại hồ sơ" secondary onPress={profile.refresh} />
          </>
        )}
        <View style={[ui.row, { flexWrap: "wrap" }]}>
          <Button
            title="Đổi ảnh đại diện"
            secondary
            loading={avatarBusy}
            disabled={busy || !profile.data}
            onPress={() => {
              void changeAvatar();
            }}
          />
          {avatar && (
            <Button
              title="Xóa ảnh đại diện"
              danger
              disabled={busy}
              onPress={() => {
                void changeAvatar(true);
              }}
            />
          )}
        </View>
        <AppText style={{ color: palette.muted, fontSize: 12 }}>
          Ảnh JPEG, PNG hoặc WebP · Tối đa 8 MB
        </AppText>
        <Field
          label="Tên hiển thị"
          value={name}
          onChangeText={setName}
          maxLength={50}
          editable={!busy && !!profile.data}
        />
        <View style={[ui.row, { flexWrap: "wrap" }]}>
          <Button
            title="Lưu hồ sơ"
            loading={busy && !avatarBusy}
            disabled={busy || !profile.data}
            onPress={() => {
              void save();
            }}
          />
          <Button
            title="Đăng xuất"
            secondary
            disabled={busy}
            onPress={() => {
              void signOut();
            }}
          />
        </View>
      </Card>
      <View style={{ gap: 18 }}>
        <SectionTitle
          title="Lịch đặt sân của tôi"
          subtitle="Booking được lấy trực tiếp từ tài khoản của bạn."
        />
        {bookings.error && bookings.data && (
          <>
            <Message error>{formError(bookings.error)}</Message>
            <Button
              title="Tải lại lịch đặt sân"
              secondary
              onPress={bookings.refresh}
            />
          </>
        )}
        {bookings.loading && !bookings.data ? (
          <StateBlock loading />
        ) : bookings.error && !bookings.data ? (
          <StateBlock error={bookings.error} retry={bookings.refresh} />
        ) : bookings.data?.bookings.length ? (
          <View style={ui.grid}>
            {bookings.data.bookings.map((booking) => (
              <Card key={booking.slot_id} style={{ width: cardWidth }}>
                <AppText style={ui.eyebrow}>
                  {sportName(booking.sport_type).toUpperCase()}
                </AppText>
                <AppText style={type.subheading}>{booking.court_name}</AppText>
                <AppText>
                  {day(booking.start_time)} · {time(booking.start_time)}–
                  {time(booking.end_time)}
                </AppText>
                {booking.game_id ? (
                  <Button
                    title="Xem trận của booking"
                    secondary
                    onPress={() =>
                      router.push({
                        pathname: "/game/[id]",
                        params: { id: booking.game_id as string },
                      })
                    }
                  />
                ) : Date.parse(booking.start_time) > Date.now() ? (
                  <>
                    <Button
                      title="Tạo trận từ lịch này"
                      icon="users"
                      onPress={() =>
                        router.push({
                          pathname: "/game/create",
                          params: {
                            slot: booking.slot_id,
                            court: booking.court_id,
                          },
                        })
                      }
                    />
                    <Button
                      title="Hủy lịch đã đặt"
                      danger
                      disabled={busy}
                      onPress={() => setConfirm(booking)}
                    />
                  </>
                ) : (
                  <AppText style={{ color: palette.muted }}>
                    Booking đã qua.
                  </AppText>
                )}
              </Card>
            ))}
          </View>
        ) : (
          <StateBlock empty="Bạn chưa có booking. Khám phá sân và chọn một khung giờ phù hợp." />
        )}
        {!!bookings.data?.total && (
          <View style={[ui.row, { flexWrap: "wrap" }]}>
            <Button
              title="Booking trước"
              secondary
              disabled={page === 1 || bookings.loading}
              onPress={() => setPage((value) => value - 1)}
            />
            <Button
              title="Booking tiếp"
              secondary
              disabled={page * 6 >= bookings.data.total || bookings.loading}
              onPress={() => setPage((value) => value + 1)}
            />
            <AppText style={{ color: palette.muted }}>Trang {page}</AppText>
          </View>
        )}
        {confirm && (
          <Card>
            <AppText>
              Hủy booking tại {confirm.court_name}, {day(confirm.start_time)}{" "}
              lúc {time(confirm.start_time)}?
            </AppText>
            <Button
              title="Xác nhận hủy lịch"
              danger
              loading={busy}
              onPress={() => {
                void cancelBooking();
              }}
            />
            <Button
              title="Giữ lịch này"
              secondary
              disabled={busy}
              onPress={() => setConfirm(null)}
            />
          </Card>
        )}
      </View>
      <View style={{ gap: 18 }}>
        <SectionTitle
          title="Các trận của tôi"
          subtitle="Trận đã tạo, đang tham gia và lịch sử có phần tiền của bạn."
        />
        {games.error && games.data && (
          <>
            <Message error>{formError(games.error)}</Message>
            <Button
              title="Tải lại các trận"
              secondary
              onPress={games.refresh}
            />
          </>
        )}
        {games.loading && !games.data ? (
          <StateBlock loading />
        ) : games.error && !games.data ? (
          <StateBlock error={games.error} retry={games.refresh} />
        ) : games.data?.games.length ? (
          <View style={ui.grid}>
            {games.data.games.map((game) => (
              <GameCard key={game.id} game={game} width={cardWidth} />
            ))}
          </View>
        ) : (
          <StateBlock empty="Bạn chưa tham gia trận nào. Tìm trận hoặc tạo trận từ booking của bạn." />
        )}
        {!!games.data?.total && (
          <View style={[ui.row, { flexWrap: "wrap" }]}>
            <Button
              title="Trận trước"
              secondary
              disabled={gamePage === 1 || games.loading}
              onPress={() => setGamePage((value) => value - 1)}
            />
            <Button
              title="Trận tiếp"
              secondary
              disabled={gamePage * 6 >= games.data.total || games.loading}
              onPress={() => setGamePage((value) => value + 1)}
            />
            <AppText style={{ color: palette.muted }}>Trang {gamePage}</AppText>
          </View>
        )}
      </View>
    </Page>
  );
}
