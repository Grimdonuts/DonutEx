local autocomplete = require("autocomplete")

if not autocomplete then
  print("autocomplete not found!")
  return
end


local cpp_keywords = {
  "alignas","alignof","and","and_eq","asm","auto","bitand","bitor","bool","break",
  "case","catch","char","char16_t","char32_t","class","compl","concept","const",
  "consteval","constexpr","constinit","const_cast","continue","co_await","co_return",
  "co_yield","decltype","default","delete","do","double","dynamic_cast","else","enum",
  "explicit","export","extern","false","float","for","friend","goto","if","inline",
  "int","long","mutable","namespace","new","noexcept","not","not_eq","nullptr",
  "operator","or","or_eq","override","private","protected","public","register",
  "reinterpret_cast","requires","return","short","signed","sizeof","static",
  "static_assert","static_cast","struct","switch","template","this","thread_local",
  "throw","true","try","typedef","typeid","typename","union","unsigned","using",
  "virtual","void","volatile","wchar_t","while","xor","xor_eq"
} -- this really isnt what we want as the final one TODO:

autocomplete.register("cpp", function(prefix)
  print("provider called, prefix=", prefix)
  local out = {}
  for _, kw in ipairs(cpp_keywords) do
    if kw:find(prefix, 1, true) == 1 then
      print(" match ->", kw)
      table.insert(out, kw)
    end
  end
  print(" returning", #out, "results")
  return out
end)
