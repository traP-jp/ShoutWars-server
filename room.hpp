#pragma once

#include <nlohmann/json.hpp>
#include <boost/uuid/time_generator_v7.hpp>
#include <boost/uuid/uuid.hpp>
#include <string>
#include <vector>
#include <memory>

class room_t {
  static boost::uuids::time_generator_v7 gen_id;

public:
  class user_t {
  public:
    const boost::uuids::uuid id = gen_id();
    std::string name;

    explicit user_t(std::string name);
  };

  const boost::uuids::uuid id = gen_id();
  const std::string version;
  const int size;

  std::vector<user_t> users;
  nlohmann::json info;

  explicit room_t(std::string version, const user_t &owner, int size);
};

class room_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  logger log_info, log_error;
  int limit;
  std::map<boost::uuids::uuid, std::shared_ptr<room_t>> rooms;

  explicit room_list_t(logger log_info, logger log_error, int limit);

  std::shared_ptr<room_t> create(const std::string &version, const room_t::user_t &owner, int size);
};
