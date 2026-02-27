// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'mcp.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

/// @nodoc
mixin _$McpComponent {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $McpComponentCopyWith<$Res> {
  factory $McpComponentCopyWith(
    McpComponent value,
    $Res Function(McpComponent) then,
  ) = _$McpComponentCopyWithImpl<$Res, McpComponent>;
}

/// @nodoc
class _$McpComponentCopyWithImpl<$Res, $Val extends McpComponent>
    implements $McpComponentCopyWith<$Res> {
  _$McpComponentCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$McpComponent_CardImplCopyWith<$Res> {
  factory _$$McpComponent_CardImplCopyWith(
    _$McpComponent_CardImpl value,
    $Res Function(_$McpComponent_CardImpl) then,
  ) = __$$McpComponent_CardImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String title, String content});
}

/// @nodoc
class __$$McpComponent_CardImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_CardImpl>
    implements _$$McpComponent_CardImplCopyWith<$Res> {
  __$$McpComponent_CardImplCopyWithImpl(
    _$McpComponent_CardImpl _value,
    $Res Function(_$McpComponent_CardImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? title = null, Object? content = null}) {
    return _then(
      _$McpComponent_CardImpl(
        title: null == title
            ? _value.title
            : title // ignore: cast_nullable_to_non_nullable
                  as String,
        content: null == content
            ? _value.content
            : content // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_CardImpl extends McpComponent_Card {
  const _$McpComponent_CardImpl({required this.title, required this.content})
    : super._();

  @override
  final String title;
  @override
  final String content;

  @override
  String toString() {
    return 'McpComponent.card(title: $title, content: $content)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_CardImpl &&
            (identical(other.title, title) || other.title == title) &&
            (identical(other.content, content) || other.content == content));
  }

  @override
  int get hashCode => Object.hash(runtimeType, title, content);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_CardImplCopyWith<_$McpComponent_CardImpl> get copyWith =>
      __$$McpComponent_CardImplCopyWithImpl<_$McpComponent_CardImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return card(title, content);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return card?.call(title, content);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (card != null) {
      return card(title, content);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return card(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return card?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (card != null) {
      return card(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Card extends McpComponent {
  const factory McpComponent_Card({
    required final String title,
    required final String content,
  }) = _$McpComponent_CardImpl;
  const McpComponent_Card._() : super._();

  String get title;
  String get content;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_CardImplCopyWith<_$McpComponent_CardImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_ButtonImplCopyWith<$Res> {
  factory _$$McpComponent_ButtonImplCopyWith(
    _$McpComponent_ButtonImpl value,
    $Res Function(_$McpComponent_ButtonImpl) then,
  ) = __$$McpComponent_ButtonImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String label, String actionId});
}

/// @nodoc
class __$$McpComponent_ButtonImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_ButtonImpl>
    implements _$$McpComponent_ButtonImplCopyWith<$Res> {
  __$$McpComponent_ButtonImplCopyWithImpl(
    _$McpComponent_ButtonImpl _value,
    $Res Function(_$McpComponent_ButtonImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? label = null, Object? actionId = null}) {
    return _then(
      _$McpComponent_ButtonImpl(
        label: null == label
            ? _value.label
            : label // ignore: cast_nullable_to_non_nullable
                  as String,
        actionId: null == actionId
            ? _value.actionId
            : actionId // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_ButtonImpl extends McpComponent_Button {
  const _$McpComponent_ButtonImpl({required this.label, required this.actionId})
    : super._();

  @override
  final String label;
  @override
  final String actionId;

  @override
  String toString() {
    return 'McpComponent.button(label: $label, actionId: $actionId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_ButtonImpl &&
            (identical(other.label, label) || other.label == label) &&
            (identical(other.actionId, actionId) ||
                other.actionId == actionId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, label, actionId);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_ButtonImplCopyWith<_$McpComponent_ButtonImpl> get copyWith =>
      __$$McpComponent_ButtonImplCopyWithImpl<_$McpComponent_ButtonImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return button(label, actionId);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return button?.call(label, actionId);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (button != null) {
      return button(label, actionId);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return button(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return button?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (button != null) {
      return button(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Button extends McpComponent {
  const factory McpComponent_Button({
    required final String label,
    required final String actionId,
  }) = _$McpComponent_ButtonImpl;
  const McpComponent_Button._() : super._();

  String get label;
  String get actionId;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_ButtonImplCopyWith<_$McpComponent_ButtonImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_MarkdownImplCopyWith<$Res> {
  factory _$$McpComponent_MarkdownImplCopyWith(
    _$McpComponent_MarkdownImpl value,
    $Res Function(_$McpComponent_MarkdownImpl) then,
  ) = __$$McpComponent_MarkdownImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String content});
}

/// @nodoc
class __$$McpComponent_MarkdownImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_MarkdownImpl>
    implements _$$McpComponent_MarkdownImplCopyWith<$Res> {
  __$$McpComponent_MarkdownImplCopyWithImpl(
    _$McpComponent_MarkdownImpl _value,
    $Res Function(_$McpComponent_MarkdownImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? content = null}) {
    return _then(
      _$McpComponent_MarkdownImpl(
        content: null == content
            ? _value.content
            : content // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_MarkdownImpl extends McpComponent_Markdown {
  const _$McpComponent_MarkdownImpl({required this.content}) : super._();

  @override
  final String content;

  @override
  String toString() {
    return 'McpComponent.markdown(content: $content)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_MarkdownImpl &&
            (identical(other.content, content) || other.content == content));
  }

  @override
  int get hashCode => Object.hash(runtimeType, content);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_MarkdownImplCopyWith<_$McpComponent_MarkdownImpl>
  get copyWith =>
      __$$McpComponent_MarkdownImplCopyWithImpl<_$McpComponent_MarkdownImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return markdown(content);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return markdown?.call(content);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (markdown != null) {
      return markdown(content);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return markdown(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return markdown?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (markdown != null) {
      return markdown(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Markdown extends McpComponent {
  const factory McpComponent_Markdown({required final String content}) =
      _$McpComponent_MarkdownImpl;
  const McpComponent_Markdown._() : super._();

  String get content;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_MarkdownImplCopyWith<_$McpComponent_MarkdownImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_RowImplCopyWith<$Res> {
  factory _$$McpComponent_RowImplCopyWith(
    _$McpComponent_RowImpl value,
    $Res Function(_$McpComponent_RowImpl) then,
  ) = __$$McpComponent_RowImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<McpComponent> children});
}

/// @nodoc
class __$$McpComponent_RowImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_RowImpl>
    implements _$$McpComponent_RowImplCopyWith<$Res> {
  __$$McpComponent_RowImplCopyWithImpl(
    _$McpComponent_RowImpl _value,
    $Res Function(_$McpComponent_RowImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? children = null}) {
    return _then(
      _$McpComponent_RowImpl(
        children: null == children
            ? _value._children
            : children // ignore: cast_nullable_to_non_nullable
                  as List<McpComponent>,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_RowImpl extends McpComponent_Row {
  const _$McpComponent_RowImpl({required final List<McpComponent> children})
    : _children = children,
      super._();

  final List<McpComponent> _children;
  @override
  List<McpComponent> get children {
    if (_children is EqualUnmodifiableListView) return _children;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_children);
  }

  @override
  String toString() {
    return 'McpComponent.row(children: $children)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_RowImpl &&
            const DeepCollectionEquality().equals(other._children, _children));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_children));

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_RowImplCopyWith<_$McpComponent_RowImpl> get copyWith =>
      __$$McpComponent_RowImplCopyWithImpl<_$McpComponent_RowImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return row(children);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return row?.call(children);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (row != null) {
      return row(children);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return row(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return row?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (row != null) {
      return row(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Row extends McpComponent {
  const factory McpComponent_Row({required final List<McpComponent> children}) =
      _$McpComponent_RowImpl;
  const McpComponent_Row._() : super._();

  List<McpComponent> get children;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_RowImplCopyWith<_$McpComponent_RowImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_ColumnImplCopyWith<$Res> {
  factory _$$McpComponent_ColumnImplCopyWith(
    _$McpComponent_ColumnImpl value,
    $Res Function(_$McpComponent_ColumnImpl) then,
  ) = __$$McpComponent_ColumnImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<McpComponent> children});
}

/// @nodoc
class __$$McpComponent_ColumnImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_ColumnImpl>
    implements _$$McpComponent_ColumnImplCopyWith<$Res> {
  __$$McpComponent_ColumnImplCopyWithImpl(
    _$McpComponent_ColumnImpl _value,
    $Res Function(_$McpComponent_ColumnImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? children = null}) {
    return _then(
      _$McpComponent_ColumnImpl(
        children: null == children
            ? _value._children
            : children // ignore: cast_nullable_to_non_nullable
                  as List<McpComponent>,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_ColumnImpl extends McpComponent_Column {
  const _$McpComponent_ColumnImpl({required final List<McpComponent> children})
    : _children = children,
      super._();

  final List<McpComponent> _children;
  @override
  List<McpComponent> get children {
    if (_children is EqualUnmodifiableListView) return _children;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_children);
  }

  @override
  String toString() {
    return 'McpComponent.column(children: $children)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_ColumnImpl &&
            const DeepCollectionEquality().equals(other._children, _children));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_children));

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_ColumnImplCopyWith<_$McpComponent_ColumnImpl> get copyWith =>
      __$$McpComponent_ColumnImplCopyWithImpl<_$McpComponent_ColumnImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return column(children);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return column?.call(children);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (column != null) {
      return column(children);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return column(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return column?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (column != null) {
      return column(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Column extends McpComponent {
  const factory McpComponent_Column({
    required final List<McpComponent> children,
  }) = _$McpComponent_ColumnImpl;
  const McpComponent_Column._() : super._();

  List<McpComponent> get children;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_ColumnImplCopyWith<_$McpComponent_ColumnImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_ImageImplCopyWith<$Res> {
  factory _$$McpComponent_ImageImplCopyWith(
    _$McpComponent_ImageImpl value,
    $Res Function(_$McpComponent_ImageImpl) then,
  ) = __$$McpComponent_ImageImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String url, String alt});
}

/// @nodoc
class __$$McpComponent_ImageImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_ImageImpl>
    implements _$$McpComponent_ImageImplCopyWith<$Res> {
  __$$McpComponent_ImageImplCopyWithImpl(
    _$McpComponent_ImageImpl _value,
    $Res Function(_$McpComponent_ImageImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? url = null, Object? alt = null}) {
    return _then(
      _$McpComponent_ImageImpl(
        url: null == url
            ? _value.url
            : url // ignore: cast_nullable_to_non_nullable
                  as String,
        alt: null == alt
            ? _value.alt
            : alt // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_ImageImpl extends McpComponent_Image {
  const _$McpComponent_ImageImpl({required this.url, required this.alt})
    : super._();

  @override
  final String url;
  @override
  final String alt;

  @override
  String toString() {
    return 'McpComponent.image(url: $url, alt: $alt)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_ImageImpl &&
            (identical(other.url, url) || other.url == url) &&
            (identical(other.alt, alt) || other.alt == alt));
  }

  @override
  int get hashCode => Object.hash(runtimeType, url, alt);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_ImageImplCopyWith<_$McpComponent_ImageImpl> get copyWith =>
      __$$McpComponent_ImageImplCopyWithImpl<_$McpComponent_ImageImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return image(url, alt);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return image?.call(url, alt);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (image != null) {
      return image(url, alt);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return image(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return image?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (image != null) {
      return image(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Image extends McpComponent {
  const factory McpComponent_Image({
    required final String url,
    required final String alt,
  }) = _$McpComponent_ImageImpl;
  const McpComponent_Image._() : super._();

  String get url;
  String get alt;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_ImageImplCopyWith<_$McpComponent_ImageImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_ProgressBarImplCopyWith<$Res> {
  factory _$$McpComponent_ProgressBarImplCopyWith(
    _$McpComponent_ProgressBarImpl value,
    $Res Function(_$McpComponent_ProgressBarImpl) then,
  ) = __$$McpComponent_ProgressBarImplCopyWithImpl<$Res>;
  @useResult
  $Res call({double progress, String label});
}

/// @nodoc
class __$$McpComponent_ProgressBarImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_ProgressBarImpl>
    implements _$$McpComponent_ProgressBarImplCopyWith<$Res> {
  __$$McpComponent_ProgressBarImplCopyWithImpl(
    _$McpComponent_ProgressBarImpl _value,
    $Res Function(_$McpComponent_ProgressBarImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? progress = null, Object? label = null}) {
    return _then(
      _$McpComponent_ProgressBarImpl(
        progress: null == progress
            ? _value.progress
            : progress // ignore: cast_nullable_to_non_nullable
                  as double,
        label: null == label
            ? _value.label
            : label // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_ProgressBarImpl extends McpComponent_ProgressBar {
  const _$McpComponent_ProgressBarImpl({
    required this.progress,
    required this.label,
  }) : super._();

  @override
  final double progress;
  @override
  final String label;

  @override
  String toString() {
    return 'McpComponent.progressBar(progress: $progress, label: $label)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_ProgressBarImpl &&
            (identical(other.progress, progress) ||
                other.progress == progress) &&
            (identical(other.label, label) || other.label == label));
  }

  @override
  int get hashCode => Object.hash(runtimeType, progress, label);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_ProgressBarImplCopyWith<_$McpComponent_ProgressBarImpl>
  get copyWith =>
      __$$McpComponent_ProgressBarImplCopyWithImpl<
        _$McpComponent_ProgressBarImpl
      >(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return progressBar(progress, label);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return progressBar?.call(progress, label);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (progressBar != null) {
      return progressBar(progress, label);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return progressBar(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return progressBar?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (progressBar != null) {
      return progressBar(this);
    }
    return orElse();
  }
}

abstract class McpComponent_ProgressBar extends McpComponent {
  const factory McpComponent_ProgressBar({
    required final double progress,
    required final String label,
  }) = _$McpComponent_ProgressBarImpl;
  const McpComponent_ProgressBar._() : super._();

  double get progress;
  String get label;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_ProgressBarImplCopyWith<_$McpComponent_ProgressBarImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$McpComponent_InputImplCopyWith<$Res> {
  factory _$$McpComponent_InputImplCopyWith(
    _$McpComponent_InputImpl value,
    $Res Function(_$McpComponent_InputImpl) then,
  ) = __$$McpComponent_InputImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String label, String fieldId});
}

/// @nodoc
class __$$McpComponent_InputImplCopyWithImpl<$Res>
    extends _$McpComponentCopyWithImpl<$Res, _$McpComponent_InputImpl>
    implements _$$McpComponent_InputImplCopyWith<$Res> {
  __$$McpComponent_InputImplCopyWithImpl(
    _$McpComponent_InputImpl _value,
    $Res Function(_$McpComponent_InputImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? label = null, Object? fieldId = null}) {
    return _then(
      _$McpComponent_InputImpl(
        label: null == label
            ? _value.label
            : label // ignore: cast_nullable_to_non_nullable
                  as String,
        fieldId: null == fieldId
            ? _value.fieldId
            : fieldId // ignore: cast_nullable_to_non_nullable
                  as String,
      ),
    );
  }
}

/// @nodoc

class _$McpComponent_InputImpl extends McpComponent_Input {
  const _$McpComponent_InputImpl({required this.label, required this.fieldId})
    : super._();

  @override
  final String label;
  @override
  final String fieldId;

  @override
  String toString() {
    return 'McpComponent.input(label: $label, fieldId: $fieldId)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$McpComponent_InputImpl &&
            (identical(other.label, label) || other.label == label) &&
            (identical(other.fieldId, fieldId) || other.fieldId == fieldId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, label, fieldId);

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$McpComponent_InputImplCopyWith<_$McpComponent_InputImpl> get copyWith =>
      __$$McpComponent_InputImplCopyWithImpl<_$McpComponent_InputImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String title, String content) card,
    required TResult Function(String label, String actionId) button,
    required TResult Function(String content) markdown,
    required TResult Function(List<McpComponent> children) row,
    required TResult Function(List<McpComponent> children) column,
    required TResult Function(String url, String alt) image,
    required TResult Function(double progress, String label) progressBar,
    required TResult Function(String label, String fieldId) input,
  }) {
    return input(label, fieldId);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String title, String content)? card,
    TResult? Function(String label, String actionId)? button,
    TResult? Function(String content)? markdown,
    TResult? Function(List<McpComponent> children)? row,
    TResult? Function(List<McpComponent> children)? column,
    TResult? Function(String url, String alt)? image,
    TResult? Function(double progress, String label)? progressBar,
    TResult? Function(String label, String fieldId)? input,
  }) {
    return input?.call(label, fieldId);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String title, String content)? card,
    TResult Function(String label, String actionId)? button,
    TResult Function(String content)? markdown,
    TResult Function(List<McpComponent> children)? row,
    TResult Function(List<McpComponent> children)? column,
    TResult Function(String url, String alt)? image,
    TResult Function(double progress, String label)? progressBar,
    TResult Function(String label, String fieldId)? input,
    required TResult orElse(),
  }) {
    if (input != null) {
      return input(label, fieldId);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(McpComponent_Card value) card,
    required TResult Function(McpComponent_Button value) button,
    required TResult Function(McpComponent_Markdown value) markdown,
    required TResult Function(McpComponent_Row value) row,
    required TResult Function(McpComponent_Column value) column,
    required TResult Function(McpComponent_Image value) image,
    required TResult Function(McpComponent_ProgressBar value) progressBar,
    required TResult Function(McpComponent_Input value) input,
  }) {
    return input(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(McpComponent_Card value)? card,
    TResult? Function(McpComponent_Button value)? button,
    TResult? Function(McpComponent_Markdown value)? markdown,
    TResult? Function(McpComponent_Row value)? row,
    TResult? Function(McpComponent_Column value)? column,
    TResult? Function(McpComponent_Image value)? image,
    TResult? Function(McpComponent_ProgressBar value)? progressBar,
    TResult? Function(McpComponent_Input value)? input,
  }) {
    return input?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(McpComponent_Card value)? card,
    TResult Function(McpComponent_Button value)? button,
    TResult Function(McpComponent_Markdown value)? markdown,
    TResult Function(McpComponent_Row value)? row,
    TResult Function(McpComponent_Column value)? column,
    TResult Function(McpComponent_Image value)? image,
    TResult Function(McpComponent_ProgressBar value)? progressBar,
    TResult Function(McpComponent_Input value)? input,
    required TResult orElse(),
  }) {
    if (input != null) {
      return input(this);
    }
    return orElse();
  }
}

abstract class McpComponent_Input extends McpComponent {
  const factory McpComponent_Input({
    required final String label,
    required final String fieldId,
  }) = _$McpComponent_InputImpl;
  const McpComponent_Input._() : super._();

  String get label;
  String get fieldId;

  /// Create a copy of McpComponent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$McpComponent_InputImplCopyWith<_$McpComponent_InputImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
