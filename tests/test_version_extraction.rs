/// 预设版本号提取功能的单元测试
///
/// 测试 extract_version_from_filename 函数的各种文件名格式
use piney::api::presets::extract_version_from_filename;

// ===== 策略1: 显式版本标记 (v/ver/version，不区分大小写) =====

#[test]
fn test_v_marker_lowercase() {
    assert_eq!(extract_version_from_filename("预设名称v1.2.3"), "1.2.3");
    assert_eq!(extract_version_from_filename("预设v1.23"), "1.23");
    assert_eq!(extract_version_from_filename("预设v1.05221"), "1.05221");
}

#[test]
fn test_v_marker_uppercase() {
    assert_eq!(extract_version_from_filename("预设名称V1.2.3"), "1.2.3");
}

#[test]
fn test_v_marker_with_separator() {
    assert_eq!(extract_version_from_filename("预设名称v_1.23"), "1.23");
    assert_eq!(extract_version_from_filename("预设名称v-1.23"), "1.23");
    assert_eq!(extract_version_from_filename("预设名称v 1.23"), "1.23");
}

#[test]
fn test_ver_marker() {
    assert_eq!(extract_version_from_filename("预设名称ver1.2.3"), "1.2.3");
    assert_eq!(extract_version_from_filename("预设名称VER1.2.3"), "1.2.3");
    assert_eq!(extract_version_from_filename("预设名称Ver_1.23"), "1.23");
}

#[test]
fn test_version_marker() {
    assert_eq!(
        extract_version_from_filename("预设名称version1.2.3"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称VERSION1.2.3"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称Version_1.23"),
        "1.23"
    );
    assert_eq!(
        extract_version_from_filename("预设名称Version 0.11.2"),
        "0.11.2"
    );
}

// ===== 策略2: 尾部版本标记 (_ver, _version) =====

#[test]
fn test_trailing_ver_marker() {
    assert_eq!(extract_version_from_filename("预设名称_1.2.3_ver"), "1.2.3");
    assert_eq!(extract_version_from_filename("预设名称-1.2.3-ver"), "1.2.3");
    assert_eq!(extract_version_from_filename("预设名称_1.2.3_VER"), "1.2.3");
}

#[test]
fn test_trailing_version_marker() {
    assert_eq!(
        extract_version_from_filename("预设名称_1.2.3_version"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称-1.2.3-version"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称_1.2.3_VERSION"),
        "1.2.3"
    );
}

// ===== 策略3: 尾部点分隔版本号 =====

#[test]
fn test_separator_underscore() {
    assert_eq!(extract_version_from_filename("预设名称_1.2.3"), "1.2.3");
}

#[test]
fn test_separator_space() {
    assert_eq!(extract_version_from_filename("预设名称 1.2.3"), "1.2.3");
}

#[test]
fn test_separator_dash() {
    assert_eq!(extract_version_from_filename("预设名称-1.2.3"), "1.2.3");
}

#[test]
fn test_non_ascii_separator() {
    // 非ASCII字符（中文）后直接跟数字
    assert_eq!(extract_version_from_filename("预设名称1.2.3"), "1.2.3");
}

// ===== 策略4: 尾部下划线分隔版本号 =====

#[test]
fn test_underscore_version() {
    assert_eq!(extract_version_from_filename("预设名称1_2_3"), "1.2.3");
}

// ===== 策略5: 中间位置版本号（版本后有其他文字） =====

#[test]
fn test_version_with_trailing_word() {
    // 版本号后面跟其他文字
    assert_eq!(
        extract_version_from_filename("预设名称_1.2.3_final"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称-1.2.3-beta"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称 1.2.3 release"),
        "1.2.3"
    );
}

#[test]
fn test_version_marker_with_trailing_word() {
    // 有显式标记 + 后面跟其他文字
    assert_eq!(
        extract_version_from_filename("预设名称v1.2.3_final"),
        "1.2.3"
    );
    assert_eq!(
        extract_version_from_filename("预设名称ver1.2.3-beta"),
        "1.2.3"
    );
}

// ===== 策略6: 纯版本号 =====

#[test]
fn test_pure_version() {
    assert_eq!(extract_version_from_filename("1.2.3"), "1.2.3");
    assert_eq!(extract_version_from_filename("0.11.2"), "0.11.2");
}

// ===== 数字预设名称 =====

#[test]
fn test_numeric_preset_name_with_dash() {
    assert_eq!(extract_version_from_filename("321-1.2.3"), "1.2.3");
}

#[test]
fn test_numeric_preset_name_with_underscore() {
    assert_eq!(extract_version_from_filename("321_1.2.3"), "1.2.3");
}

#[test]
fn test_numeric_preset_name_with_space() {
    assert_eq!(extract_version_from_filename("321 1.2.3"), "1.2.3");
}

#[test]
fn test_numeric_preset_name_with_v() {
    assert_eq!(extract_version_from_filename("321v1.2.3"), "1.2.3");
}

// ===== 各种版本号格式 =====

#[test]
fn test_two_segment_version() {
    assert_eq!(extract_version_from_filename("预设v1.23"), "1.23");
    assert_eq!(extract_version_from_filename("预设v1.0023"), "1.0023");
}

#[test]
fn test_three_segment_version() {
    assert_eq!(extract_version_from_filename("预设v0.11.2"), "0.11.2");
}

// ===== 无版本号（应返回默认值） =====

#[test]
fn test_no_version_chinese() {
    assert_eq!(extract_version_from_filename("普通预设名称"), "1.0.0");
}

#[test]
fn test_no_version_short() {
    assert_eq!(extract_version_from_filename("预设"), "1.0.0");
}

#[test]
fn test_no_version_pure_digits() {
    // 纯数字没有分隔符 → 不被识别为版本号
    assert_eq!(extract_version_from_filename("12345"), "1.0.0");
}

// ===== 真实世界文件名 =====

#[test]
fn test_real_filename_zhongshengxiang() {
    assert_eq!(
        extract_version_from_filename("【众生相】Gemini-v1.0"),
        "1.0"
    );
}

#[test]
fn test_real_filename_moli() {
    // 版本号在标记前面: 0.0521ver
    assert_eq!(
        extract_version_from_filename("茉莉】《茉莉文集》0.0521ver"),
        "0.0521"
    );
}

#[test]
fn test_real_filename_mom() {
    // 版本号被中文字符包围，无显式分隔符
    assert_eq!(
        extract_version_from_filename("【MoM】果实5.40丨喵喵电波@KKM@YUKI"),
        "5.40"
    );
}
