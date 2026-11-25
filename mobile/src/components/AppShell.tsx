import React, { useEffect, useState } from "react";
import {
  Keyboard,
  Platform,
  View,
  useWindowDimensions,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { Link, usePathname, router } from "expo-router";
import { useSession } from "../stores/session";
import { AppText, Button, Icon, Message, palette, ui } from "./ui";

const destinations = [
  { href: "/", label: "Khám phá", icon: "compass" },
  { href: "/games", label: "Trận đấu", icon: "users" },
  { href: "/saved", label: "Đã lưu", icon: "heart" },
  { href: "/profile", label: "Tài khoản", icon: "user" },
] as const;
function titleForPath(path: string) {
  if (path === "/") return "PickUp — Ra sân, nhập hội.";
  if (path.startsWith("/court/")) return "Chi tiết sân · PickUp";
  if (path.startsWith("/game/create")) return "Tạo trận · PickUp";
  if (path.startsWith("/game/")) return "Chi tiết trận · PickUp";
  const titles: Record<string, string> = {
    "/explore": "Khám phá sân · PickUp",
    "/games": "Tìm đồng đội · PickUp",
    "/saved": "Sân đã lưu · PickUp",
    "/profile": "Tài khoản · PickUp",
    "/login": "Đăng nhập · PickUp",
    "/register": "Tạo tài khoản · PickUp",
    "/reset-password": "Đặt lại mật khẩu · PickUp",
  };
  return titles[path] || "Không tìm thấy trang · PickUp";
}
export function AppShell({ children }: { children: React.ReactNode }) {
  const { width } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const path = usePathname();
  const { session, storageError, clearStorageError } = useSession();
  const [keyboardVisible, setKeyboardVisible] = useState(false);
  useEffect(() => {
    if (Platform.OS === "web") return;
    const show = Keyboard.addListener("keyboardDidShow", () =>
      setKeyboardVisible(true),
    );
    const hide = Keyboard.addListener("keyboardDidHide", () =>
      setKeyboardVisible(false),
    );
    return () => {
      show.remove();
      hide.remove();
    };
  }, []);
  const desktop = width >= 850;
  const auth = ["/login", "/register", "/reset-password"].includes(path);
  useEffect(() => {
    if (Platform.OS !== "web" || typeof document === "undefined") return;
    document.title = titleForPath(path);
  }, [path]);
  return (
    <View
      style={{
        flex: 1,
        backgroundColor: palette.paper,
        paddingTop: insets.top,
      }}
    >
      <View
        style={{
          backgroundColor: palette.paper,
          borderBottomWidth: 1,
          borderColor: palette.line,
        }}
      >
        <View
          style={[
            ui.row,
            {
              width: "100%",
              maxWidth: 1240,
              alignSelf: "center",
              paddingHorizontal: desktop ? 40 : 20,
              height: desktop ? 88 : 72,
              justifyContent: "space-between",
            },
          ]}
        >
          <Link
            href="/"
            accessibilityRole="link"
            accessibilityLabel="PickUp trang chủ"
            style={[ui.row, { minHeight: 48 }]}
          >
            <View
              style={{
                width: 36,
                height: 36,
                backgroundColor: palette.forest,
                borderRadius: 12,
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Icon name="activity" color={palette.lime} size={22} />
            </View>
            <AppText
              maxFontSizeMultiplier={1}
              style={{
                fontFamily: "Inter_700Bold",
                fontSize: 27,
                letterSpacing: -1.3,
              }}
            >
              pick
              <AppText
                maxFontSizeMultiplier={1}
                style={{
                  fontFamily: "Inter_700Bold",
                  fontSize: 27,
                  color: palette.green,
                }}
              >
                up.
              </AppText>
            </AppText>
          </Link>
          {desktop && (
            <View style={[ui.row, { gap: 8 }]}>
              {destinations.slice(0, 3).map((item) => (
                <Link
                  key={item.href}
                  href={item.href}
                  accessibilityRole="link"
                  accessibilityState={{ selected: path === item.href }}
                  style={{
                    paddingHorizontal: 18,
                    minHeight: 48,
                    justifyContent: "center",
                    borderRadius: 25,
                    backgroundColor:
                      path === item.href ? palette.pale : "transparent",
                  }}
                >
                  <AppText
                    style={{
                      fontFamily: "Inter_500Medium",
                      color: path === item.href ? palette.ink : palette.muted,
                    }}
                  >
                    {item.label}
                  </AppText>
                </Link>
              ))}
            </View>
          )}
          {desktop ? (
            <Button
              title={session ? "Tài khoản" : "Đăng nhập"}
              secondary
              icon="user"
              onPress={() => router.push(session ? "/profile" : "/login")}
              style={{ minHeight: 48, maxWidth: 220 }}
            />
          ) : (
            <View style={[ui.row, { flexShrink: 1 }]}>
              <Icon name="map-pin" size={14} />
              <AppText style={{ color: palette.muted, fontSize: 11, flexShrink: 1 }}>
                Hồ Chí Minh
              </AppText>
            </View>
          )}
        </View>
      </View>
      {storageError && (
        <View style={{ paddingHorizontal: desktop ? 40 : 20, paddingTop: 12, gap: 8 }}>
          <Message error>{storageError}</Message>
          <Button title="Đóng thông báo" secondary onPress={clearStorageError} />
        </View>
      )}
      <View style={{ flex: 1, minHeight: 0 }}>{children}</View>
      {!desktop && !auth && !keyboardVisible && (
        <View
          style={{
            backgroundColor: palette.white,
            borderTopWidth: 1,
            borderColor: palette.line,
            paddingBottom: Math.max(10, insets.bottom),
            paddingTop: 9,
            flexDirection: "row",
          }}
        >
          {destinations.map((item) => {
            const selected = path === item.href;
            return (
              <Link
                key={item.href}
                href={item.href}
                accessibilityRole="link"
                accessibilityLabel={item.label}
                accessibilityState={{ selected }}
                style={{
                  flex: 1,
                  minHeight: 50,
                  marginHorizontal: 4,
                  paddingVertical: 4,
                  borderRadius: 16,
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 5,
                  backgroundColor: selected ? palette.pale : "transparent",
                }}
              >
                <Icon
                  name={item.icon}
                  size={21}
                  color={selected ? palette.green : palette.muted}
                />
                <AppText
                  style={{
                    fontSize: 10,
                    lineHeight: 15,
                    fontFamily: selected
                      ? "Inter_600SemiBold"
                      : "Inter_400Regular",
                    color: selected ? palette.green : palette.muted,
                  }}
                >
                  {item.label}
                </AppText>
              </Link>
            );
          })}
        </View>
      )}
    </View>
  );
}
