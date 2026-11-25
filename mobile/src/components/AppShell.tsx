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
import { ActivePill, MotionView, RouteTransition } from "./motion";

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
      <MotionView
        distance={-12}
        duration={520}
        style={{
          backgroundColor: palette.paper,
          borderBottomWidth: 1,
          borderColor: palette.line,
          shadowColor: palette.forest,
          shadowOpacity: 0.06,
          shadowRadius: 18,
          shadowOffset: { width: 0, height: 6 },
          zIndex: 10,
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
          <MotionView distance={-8} duration={480}>
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
          </MotionView>
          {desktop && (
            <MotionView delay={70} duration={500} style={[ui.row, { gap: 8 }]}>
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
                    backgroundColor: "transparent",
                    position: "relative",
                    overflow: "hidden",
                  }}
                >
                  <ActivePill selected={path === item.href} color={palette.pale} />
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
            </MotionView>
          )}
          {desktop ? (
            <MotionView delay={130} duration={500} distance={-8} style={{ alignSelf: "flex-end" }}>
            <Button
              title={session ? "Tài khoản" : "Đăng nhập"}
              secondary
              icon="user"
              onPress={() => router.push(session ? "/profile" : "/login")}
              style={{ minHeight: 48, maxWidth: 220 }}
            />
            </MotionView>
          ) : (
            <View style={[ui.row, { flexShrink: 1 }]}>
              <Icon name="map-pin" size={14} />
              <AppText style={{ color: palette.muted, fontSize: 11, flexShrink: 1 }}>
                Hồ Chí Minh
              </AppText>
            </View>
          )}
        </View>
      </MotionView>
      {storageError && (
        <View style={{ paddingHorizontal: desktop ? 40 : 20, paddingTop: 12, gap: 8 }}>
          <Message error>{storageError}</Message>
          <Button title="Đóng thông báo" secondary onPress={clearStorageError} />
        </View>
      )}
      <RouteTransition routeKey={path}>{children}</RouteTransition>
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
          {destinations.map((item, index) => {
            const selected = path === item.href;
            return (
              <MotionView key={item.href} delay={140 + index * 55} duration={460} distance={10} style={{ flex: 1 }}>
              <Link
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
                  backgroundColor: "transparent",
                  position: "relative",
                  overflow: "hidden",
                }}
              >
                <ActivePill selected={selected} radius={16} color={palette.pale} />
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
              </MotionView>
            );
          })}
        </View>
      )}
    </View>
  );
}
