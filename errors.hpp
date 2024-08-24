#pragma once

#include <stdexcept>
#include <string>

class error : public std::runtime_error {
public:
  const int code;
  explicit error(int code, const std::string &what) : runtime_error(what), code(code) {}
};

class bad_request_error final : public error {
public:
  explicit bad_request_error(const std::string &what) : error(400, what) {}
};

class unauthorized_error final : public error {
public:
  explicit unauthorized_error(const std::string &what) : error(401, what) {}
};

class forbidden_error final : public error {
public:
  explicit forbidden_error(const std::string &what) : error(403, what) {}
};

class not_found_error final : public error {
public:
  explicit not_found_error(const std::string &what) : error(404, what) {}
};

class too_many_requests_error final : public error {
public:
  explicit too_many_requests_error(const std::string &what) : error(429, what) {}
};

class internal_server_error final : public error {
public:
  explicit internal_server_error(const std::string &what) : error(500, what) {}
};

class service_unavailable_error final : public error {
public:
  explicit service_unavailable_error(const std::string &what) : error(503, what) {}
};
